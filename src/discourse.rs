use crate::lens::Finding;
use crate::llm::Llm;
use crate::par_map;
use crate::spec::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// docs/design-spec.md §4: CHALLENGE는 "다른 방법론/다른 소스로 재측정해 수치·주장 불일치를
/// 제기"하는 경우로만 유효 인정(원본 codereview-loop의 "근거·반례·범위 등 반박" 규칙보다 좁힘).
/// 근거 없는 취향 반박("오래된 것 같다")은 SURFACE로 강등하도록 프롬프트에 명시.
///
/// #2: 예전엔 라운드에 CHALLENGE가 하나도 없으면 코드가 자동으로 같은 프롬프트를 재호출했다 —
/// 반례가 없는 게 정상인 상황에서도 모델이 형식을 맞추려고 반박을 지어낼 위험이 있어 제거했다.
/// CHALLENGE 0건은 이제 정상 상태로 취급한다(아래 프롬프트도 "최소 1회" 요구를 뺐다).
///
/// #1: 발언(moves)과 판정(resolutions)을 별도 LLM 호출로 분리한다 — 이제 렌즈별 독립 critic
/// 호출 — #1 완전 해결. moves 단계 자체가 "참여 렌즈 수만큼의 독립 호출"로 나뉜다: 각 렌즈는
/// 자기 자신이 낸 finding은 보지 못하고 다른 렌즈들의 finding만 심사한다(자기 finding 자기 심사 방지).
/// 렌즈별 호출은 서로의 결과를 모르는 채 병렬 실행되고(par_map), 모인 moves를 판정(resolutions)
/// 호출 1회가 최종 판정한다. [`run_round_call`] 참조.
pub const DISCOURSE_MOVES_SYSTEM: &str = "당신은 여러 애널리스트의 finding을 검토하는 반박자(critic) 역할이다. \
이 호출에서는 최종 판정(CONFIRMED/REJECTED/MERGED/UNCERTAIN)을 내리지 않는다 — 그건 별도의 판정자 호출이 한다. \
내용 없는 동의나 반박은 하지 않는다. AGREE는 새로운 인용/근거가 있을 때만 사용한다. \
CHALLENGE는 '동일 지표를 다른 방법론이나 다른 독립 소스로 재측정해 수치·주장 불일치를 제기'하는 \
경우로만 인정한다 — 근거 없이 '오래된 것 같다', '톤이 별로다' 같은 취향성 반박은 CHALLENGE가 아니라 SURFACE로 제기한다. \
이번 라운드에 CHALLENGE가 하나도 없어도 정상이다 — 반박할 근거가 없다면 형식을 맞추려고 억지로 만들어내지 않는다. \
AGREE/CHALLENGE에는 주장 강도에 따른 confidence(high|medium|low)를 반드시 명시한다. \
반드시 지정된 JSON 스키마로만 응답한다.";

/// 판정 전용 호출의 시스템 프롬프트(#1). 이 호출은 새 move를 만들지 않고, moves 호출이 이미
/// 만들어낸 근거만 갖고 CONFIRMED/REJECTED/MERGED/UNCERTAIN을 정한다 — "자신이 낸 반박을
/// 자신이 판정"하는 구조를 최소한으로나마 분리하기 위함.
pub const DISCOURSE_ADJUDICATE_SYSTEM: &str = "당신은 다른 애널리스트들이 이미 제기한 moves(AGREE/CHALLENGE/CONNECT/SURFACE)를 \
근거로 각 finding의 최종 판정만 내리는 판정자(adjudicator)다. 새로운 move를 만들지 않고, 주어진 moves 목록에 없는 \
근거를 지어내지 않는다. moves가 뒷받침하지 않으면 UNCERTAIN으로 남긴다. \
반드시 지정된 JSON 스키마로만 응답한다.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Move {
    #[serde(rename = "move")]
    pub kind: String, // AGREE|CHALLENGE|CONNECT|SURFACE
    pub lens: String,
    pub target: String,
    pub detail: String,
    #[serde(default)]
    pub new_evidence: String,
    #[serde(default)]
    pub confidence: String, // high|medium|low (AGREE/CHALLENGE에만 의미 있음)
}

/// ReConcile식 confidence bucket → 가중치. 라운드 소진 후 잔여 UNCERTAIN을
/// 판정 없이 버리는 대신 AGREE/CHALLENGE 누적으로 최종 판정한다.
///
/// #3: 이 1.0/0.6/0.3 상수는 LLM이 자기신고한 high/medium/low를 그대로 옮긴 값이며,
/// 실측 정확도로 보정(calibration)된 값이 아니다 — 실제 정확도를 측정하려면 렌즈·모델·오류유형별
/// benchmark 데이터(예: "high로 표시된 CHALLENGE 중 실제로 맞았던 비율")가 필요한데, 이 저장소에는
/// 그런 라벨링된 벤치마크가 없다. 통계적 calibration 자체는 이번 스코프에서 하지 않았다(이슈 코멘트 참조).
/// 근거 없는 상수(예전 high=1.0/medium=0.6/low=0.3)로 차등 가중하는 건 실측 없이 정밀도를
/// 가장하는 것이라, calibration 데이터가 생기기 전까지는 모든 move를 동일 가중치로 둔다
/// (self-reported label이 실제 정확도와 상관관계가 있다는 근거가 없으므로 균등 가중이 더 안전한 기본값).
/// 추가로 이 값이 "hard evidence"(checks.rs의 결정론적 FAIL)를 절대 뒤집을 수 없도록
/// quantify.rs::verdict()에서 결정론 체크 FAIL을 findings/confidence와 무관한 독립 조건으로
/// 강제한다(quantify.rs의 hard_evidence_fail 우선순위 참고, 테스트로 고정).
fn confidence_weight(_c: &str) -> f64 {
    1.0
}

const VOTE_THRESHOLD: f64 = 0.6;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resolution {
    pub finding_id: String,
    pub status: String, // CONFIRMED|REJECTED|MERGED|UNCERTAIN
    #[serde(default)]
    pub merged_into: String,
    #[serde(default)]
    pub reason: String,
    /// #7: --prior 재검사에서 fix check가 UNKNOWN(확인 불가)이나 REVERSED(뒤집힘)로 판정한
    /// finding에 세운다. UNKNOWN을 FIXED처럼 조용히 해제하지 않고, 사람이 직접 확인해야 함을
    /// report.rs/quantify.rs가 명시적으로 드러내기 위한 플래그.
    #[serde(default)]
    pub needs_human_review: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct DiscourseRound {
    #[serde(default)]
    moves: Vec<Move>,
    #[serde(default)]
    resolutions: Vec<Resolution>,
    #[serde(default)]
    surfaced: Vec<Finding>,
}

pub struct DiscourseAudit {
    pub round: usize,
    pub moves: Vec<Move>,
}

/// lens/reviewer는 의도적으로 노출하지 않는다 — 어떤 페르소나가 냈는지 알면
/// discourse가 근거가 아니라 "권위"로 기울 수 있다(담합/편향 연구 근거, codereview-loop 상속).
fn findings_catalog(findings: &[Finding], resolved: &HashMap<String, Resolution>) -> String {
    findings
        .iter()
        .map(|f| {
            let status = resolved
                .get(&f.id)
                .map(|r| r.status.as_str())
                .unwrap_or("UNRESOLVED");
            format!(
                "- id={} | 섹션={} | 인용={} | severity={} | label={} | citation_status={} | status={}\n  주장: {}\n  근거: {}",
                f.id, f.section, f.citation_ref, f.severity, f.label, f.citation_status, status, f.claim, f.evidence
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 이번 라운드 검토 대상(미해결 또는 이전 라운드 UNCERTAIN) finding이 속한 렌즈 목록을
/// 등장 순서대로 중복 없이 구한다(#1) — 각 렌즈마다 별도 critic 호출을 만들기 위한 참여자 명단.
fn participating_lenses(findings: &[Finding], resolved: &HashMap<String, Resolution>) -> Vec<String> {
    let mut lenses: Vec<String> = Vec::new();
    for f in findings {
        let reviewable = resolved.get(&f.id).map(|r| r.status == "UNCERTAIN").unwrap_or(true);
        if reviewable && !f.lens.is_empty() && !lenses.contains(&f.lens) {
            lenses.push(f.lens.clone());
        }
    }
    lenses
}

/// 1단계(발언) 프롬프트를 렌즈 1개(`acting_lens`) 전용으로 만든다(#1 완전 해결).
/// `other_findings`는 호출부(`run_lens_critic_call`)가 이미 `acting_lens` 소속 finding을
/// 걸러낸 목록이다 — 이 함수는 그 결과를 그대로 카탈로그화할 뿐, 자기 finding을 다시 섞지 않는다.
fn build_moves_prompt_for_lens(
    spec: &Spec,
    other_findings: &[Finding],
    resolved: &HashMap<String, Resolution>,
    round: usize,
    acting_lens: &str,
) -> String {
    let persona = spec
        .lens_by_id(acting_lens)
        .map(|l| format!("{}({})", l.title, l.persona_voice))
        .unwrap_or_else(|| acting_lens.to_string());
    format!(
        "# 과제\n라운드 {round} discourse의 1단계(발언)를 렌즈별 독립 critic으로 수행한다. \
         당신은 렌즈 '{acting_lens}' — {persona} 관점의 critic이다. 아래 목록은 이미 당신 자신의 렌즈가 \
         낸 finding을 제외한 '다른 렌즈들'의 finding만이다 — 자기 자신의 finding을 스스로 심사하는 일이 \
         구조적으로 불가능하도록 애초에 제외됐다. 이 단계에서는 판정(CONFIRMED/REJECTED 등)을 내리지 \
         않는다 — 판정은 모든 렌즈의 moves를 모은 뒤 이어지는 별도 adjudicator 호출이 수행한다.\n\n\
         ## 검토 대상 finding(다른 렌즈들, 미해결 상태만 새로 발언 대상)\n{catalog}\n\n\
         ## 규칙\n\
         - 각 move는 AGREE/CHALLENGE/CONNECT/SURFACE 중 하나, target에 finding id 명시. lens 필드는 '{acting_lens}'로 채운다.\n\
         - AGREE: 대상 finding에 없던 새 근거(new_evidence, 독립 소스에서 같은 수치·주장을 재확인)가 있을 때만. confidence 필수.\n\
         - CHALLENGE: 반드시 '다른 방법론/다른 소스로 재측정한 불일치'만 인정(취향 반박 금지). confidence 필수. 없으면 없는 대로 둔다 — 강제 아님.\n\
         - CONNECT: 둘 이상의 finding id를 detail에 명시하며 서로 다른 렌즈의 발견을 연결(예: 재무 발견 ↔ 인센티브 발견).\n\
         - SURFACE: 새 finding을 surfaced 배열에 근거와 함께 추가. 근거 없는 반박도 여기로.\n\
         - confidence는 AGREE/CHALLENGE에서만: 주장의 근거 강도가 강하면 high, 보통이면 medium, 약하면 low.\n\
         - 내용 없는 동의/반박은 만들지 말 것.\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"moves\":[{{\"move\":\"AGREE|CHALLENGE|CONNECT|SURFACE\",\"lens\":\"{acting_lens}\",\"target\":\"finding id\",\
         \"detail\":\"...\",\"new_evidence\":\"...\",\"confidence\":\"high|medium|low\"}}],\
         \"surfaced\":[{{\"section\":\"...\",\"citation_ref\":\"...\",\"claim\":\"...\",\"evidence\":\"...\",\"impact\":\"...\",\
         \"severity\":\"P0|P1|P2|P3\",\"label\":<허용값 중 하나>,\"confidence\":\"high|medium|low\",\"recommendation\":\"...\",\
         \"citation_status\":\"VERIFIED|UNVERIFIED|STALE|CONTRADICTED\"}}]}}\n",
        round = round,
        acting_lens = acting_lens,
        persona = persona,
        catalog = findings_catalog(other_findings, resolved),
    )
}

fn moves_catalog(moves: &[Move]) -> String {
    if moves.is_empty() {
        return "(이번 라운드 move 없음 — CHALLENGE 0건은 정상이다)".to_string();
    }
    moves
        .iter()
        .map(|m| {
            format!(
                "- [{}] lens={} target={} confidence={} — {} (new_evidence: {})",
                m.kind, m.lens, m.target, m.confidence, m.detail, m.new_evidence
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// 2단계(판정) 프롬프트 — 1단계에서 이미 확정된 moves만 입력으로 받는다. 이 호출은 새 move를
/// 만들 권한이 없고, 오직 주어진 moves를 근거로 CONFIRMED/REJECTED/MERGED/UNCERTAIN만 정한다(#1).
fn build_resolutions_prompt(findings: &[Finding], resolved: &HashMap<String, Resolution>, round: usize, moves: &[Move]) -> String {
    format!(
        "# 과제\n라운드 {round} discourse의 2단계(판정)를 수행한다. 아래는 다른 애널리스트들이 이번 라운드에 \
         이미 제기한 moves다 — 당신은 새 move를 만들지 않고, 이 moves만 근거로 각 finding의 최종 판정을 내린다.\n\n\
         ## 전체 findings (미해결 상태만 새로 판정 대상)\n{catalog}\n\n\
         ## 이번 라운드 moves(판정 근거, 이미 확정됨 — 수정 불가)\n{moves}\n\n\
         ## 규칙\n\
         - resolutions는 UNRESOLVED 또는 이전 라운드 UNCERTAIN이었던 finding만 판정: CONFIRMED|REJECTED|MERGED|UNCERTAIN.\n\
         - moves가 뒷받침하지 않는 판정은 하지 않는다 — 근거가 부족하면 UNCERTAIN으로 남긴다.\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"resolutions\":[{{\"finding_id\":\"...\",\"status\":\"CONFIRMED|REJECTED|MERGED|UNCERTAIN\",\
         \"merged_into\":\"\",\"reason\":\"...\"}}]}}\n",
        round = round,
        catalog = findings_catalog(findings, resolved),
        moves = moves_catalog(moves),
    )
}

/// discourse 라운드 반복. 미해결/UNCERTAIN finding이 없어지거나 max_rounds에 도달하면 종료.
///
/// #2: 예전엔 CHALLENGE가 0건인 라운드를 코드가 자동으로 재요청했다 — 반례가 없는 게 정상인
/// 상황에서 모델이 형식을 맞추려고 반박을 지어낼 위험이 있어 제거했다. CHALLENGE 0건은 이제
/// 그대로 받아들인다(재호출 없음).
///
/// #1: [`run_round_call`]이 발언(moves)을 렌즈별 독립 critic 호출로 나누고, 판정(resolutions)은
/// 그 결과를 모두 모은 뒤 별도 adjudicator 호출 1회로 수행한다 — 이 함수 자체는 그 결과를 받아
/// 취합하기만 하므로 로직은 이전과 동일하다. `concurrency`는 렌즈별 critic 호출을 병렬 실행하는 데
/// 쓴다(main.rs `par_map`, 렌즈 리뷰 단계와 동일 인프라 재사용).
pub fn run(
    llm: &Llm,
    spec: &Spec,
    findings: &mut Vec<Finding>,
    max_rounds: usize,
    concurrency: usize,
) -> Result<(Vec<DiscourseAudit>, HashMap<String, Resolution>)> {
    let max_rounds = max_rounds.max(1);
    let mut resolved: HashMap<String, Resolution> = HashMap::new();
    let mut audit: Vec<DiscourseAudit> = Vec::new();

    for round in 1..=max_rounds {
        let unresolved = findings
            .iter()
            .any(|f| resolved.get(&f.id).map(|r| r.status == "UNCERTAIN").unwrap_or(true));
        if !unresolved {
            break;
        }

        let mut dr = run_round_call(llm, spec, findings, &resolved, round, concurrency)?;

        for (i, sf) in dr.surfaced.iter_mut().enumerate() {
            sf.id = format!("surface-r{}-{}", round, i + 1);
            if sf.lens.is_empty() {
                sf.lens = "discourse".to_string();
            }
            if sf.citation_ref.trim().is_empty() {
                sf.citation_ref = "UNKNOWN".to_string();
            }
        }
        findings.extend(dr.surfaced.clone());

        for r in dr.resolutions.clone() {
            resolved.insert(r.finding_id.clone(), r);
        }

        audit.push(DiscourseAudit { round, moves: dr.moves });

        if round == max_rounds {
            break;
        }
    }

    // 라운드 소진 후 남은 UNCERTAIN/미판정 finding: confidence-weighted vote로 최종 판정.
    for f in findings.iter() {
        let still_uncertain = resolved
            .get(&f.id)
            .map(|r| r.status == "UNCERTAIN")
            .unwrap_or(true);
        if !still_uncertain {
            continue;
        }

        let net: f64 = audit
            .iter()
            .flat_map(|a| a.moves.iter())
            .filter(|m| m.target == f.id)
            .map(|m| match m.kind.as_str() {
                "AGREE" => confidence_weight(&m.confidence),
                "CHALLENGE" => -confidence_weight(&m.confidence),
                _ => 0.0,
            })
            .sum();

        let (status, reason) = if net >= VOTE_THRESHOLD {
            ("CONFIRMED".to_string(), format!("discourse 라운드 소진, confidence-weighted vote로 확정(net={net:.2})"))
        } else if net <= -VOTE_THRESHOLD {
            ("REJECTED".to_string(), format!("discourse 라운드 소진, confidence-weighted vote로 기각(net={net:.2})"))
        } else {
            ("UNCERTAIN".to_string(), format!("discourse 라운드 소진, 판정 없음(net={net:.2})"))
        };

        resolved.insert(
            f.id.clone(),
            Resolution { finding_id: f.id.clone(), status, merged_into: String::new(), reason, needs_human_review: false },
        );
    }

    Ok((audit, resolved))
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MovesRound {
    #[serde(default)]
    moves: Vec<Move>,
    #[serde(default)]
    surfaced: Vec<Finding>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct ResolutionsRound {
    #[serde(default)]
    resolutions: Vec<Resolution>,
}

/// 렌즈별 독립 critic 호출(#1) — `acting_lens`는 findings에서 자기 자신이 낸 것을 제외한
/// "다른 렌즈들"의 finding만 보고 moves/surfaced를 생성한다. 결과 move의 lens는 항상
/// `acting_lens`로 고정한다(모델이 lens 필드를 잘못 채우거나 비워도 귀속이 흔들리지 않도록).
fn run_lens_critic_call(
    llm: &Llm,
    spec: &Spec,
    findings: &[Finding],
    resolved: &HashMap<String, Resolution>,
    round: usize,
    acting_lens: &str,
) -> Result<MovesRound> {
    let others: Vec<Finding> = findings.iter().filter(|f| f.lens != acting_lens).cloned().collect();
    let prompt = build_moves_prompt_for_lens(spec, &others, resolved, round, acting_lens);
    let mv = llm
        .json(&prompt, Some(DISCOURSE_MOVES_SYSTEM))
        .with_context(|| format!("discourse 라운드 {round} 렌즈 '{acting_lens}' critic 호출 실패"))?;
    let mut mr: MovesRound = serde_json::from_value(mv)
        .with_context(|| format!("discourse 라운드 {round} 렌즈 '{acting_lens}' moves JSON 스키마 불일치"))?;
    for m in mr.moves.iter_mut() {
        m.lens = acting_lens.to_string();
    }
    Ok(mr)
}

/// 라운드 1회 실행 — 이제 렌즈별 독립 critic 호출: #1 완전 해결.
/// 1) 이번 라운드 검토 대상 finding을 소속 렌즈별로 그룹화해([`participating_lenses`]) 참여 렌즈를 구한다.
/// 2) 참여 렌즈가 2개 미만(비교 대상 없음)이면 critic 단계를 생략한다(moves 없이 진행).
/// 3) 참여 렌즈마다 [`run_lens_critic_call`]을 서로의 결과를 모르는 채 독립 호출한다 — 호출 수가
///    렌즈 수만큼 늘어나므로 `concurrency`만큼 병렬 실행한다(`par_map`, 렌즈 리뷰 단계와 동일 인프라).
/// 4) 모든 렌즈의 moves를 모아 DISCOURSE_ADJUDICATE_SYSTEM 판정 호출 1회로 최종 판정한다(기존 로직 유지,
///    입력만 렌즈별 moves를 합친 것으로 바뀜).
fn run_round_call(
    llm: &Llm,
    spec: &Spec,
    findings: &[Finding],
    resolved: &HashMap<String, Resolution>,
    round: usize,
    concurrency: usize,
) -> Result<DiscourseRound> {
    let lenses = participating_lenses(findings, resolved);

    let (all_moves, all_surfaced): (Vec<Move>, Vec<Finding>) = if lenses.len() < 2 {
        // 렌즈가 0~1개면 비교 대상이 없다 — critic 호출 자체를 스킵(그 렌즈는 스킵).
        (Vec::new(), Vec::new())
    } else {
        let results: Vec<MovesRound> = par_map(concurrency, lenses, |acting_lens| {
            run_lens_critic_call(llm, spec, findings, resolved, round, &acting_lens)
        })?;
        let mut moves = Vec::new();
        let mut surfaced = Vec::new();
        for mr in results {
            moves.extend(mr.moves);
            surfaced.extend(mr.surfaced);
        }
        (moves, surfaced)
    };

    let res_prompt = build_resolutions_prompt(findings, resolved, round, &all_moves);
    let rv = llm
        .json(&res_prompt, Some(DISCOURSE_ADJUDICATE_SYSTEM))
        .with_context(|| format!("discourse 라운드 {round} resolutions 단계 실패"))?;
    let rr: ResolutionsRound =
        serde_json::from_value(rv).with_context(|| format!("discourse 라운드 {round} resolutions JSON 스키마 불일치"))?;

    Ok(DiscourseRound { moves: all_moves, resolutions: rr.resolutions, surfaced: all_surfaced })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::Lens;

    fn finding(id: &str, lens: &str) -> Finding {
        Finding {
            id: id.to_string(),
            section: "sec".to_string(),
            citation_ref: "1".to_string(),
            claim: format!("claim-{id}"),
            evidence: format!("evidence-{id}"),
            impact: String::new(),
            severity: "P2".to_string(),
            label: "x".to_string(),
            confidence: "medium".to_string(),
            recommendation: String::new(),
            lens: lens.to_string(),
            reviewer: String::new(),
            citation_status: "UNVERIFIED".to_string(),
            llm_citation_status: String::new(),
        }
    }

    fn test_spec() -> Spec {
        Spec {
            name: "test".to_string(),
            context: String::new(),
            lenses: vec![
                Lens {
                    id: "lens_a".to_string(),
                    title: "Lens A".to_string(),
                    guide: String::new(),
                    always: false,
                    signal: String::new(),
                    persona_name: String::new(),
                    persona_voice: "A 관점".to_string(),
                    tier: String::new(),
                },
                Lens {
                    id: "lens_b".to_string(),
                    title: "Lens B".to_string(),
                    guide: String::new(),
                    always: false,
                    signal: String::new(),
                    persona_name: String::new(),
                    persona_voice: "B 관점".to_string(),
                    tier: String::new(),
                },
            ],
            labels: vec!["x".to_string()],
            subject_owned_domains: Vec::new(),
            staleness_threshold_years: 0,
            enabled_checks: Vec::new(),
        }
    }

    #[test]
    fn participating_lenses_dedupes_and_skips_resolved() {
        let findings = vec![finding("f1", "lens_a"), finding("f2", "lens_b"), finding("f3", "lens_a")];
        let mut resolved = HashMap::new();
        resolved.insert(
            "f3".to_string(),
            Resolution { finding_id: "f3".to_string(), status: "CONFIRMED".to_string(), merged_into: String::new(), reason: String::new(), needs_human_review: false },
        );
        let lenses = participating_lenses(&findings, &resolved);
        assert_eq!(lenses, vec!["lens_a".to_string(), "lens_b".to_string()]);
    }

    #[test]
    fn participating_lenses_empty_when_all_resolved() {
        let findings = vec![finding("f1", "lens_a")];
        let mut resolved = HashMap::new();
        resolved.insert(
            "f1".to_string(),
            Resolution { finding_id: "f1".to_string(), status: "CONFIRMED".to_string(), merged_into: String::new(), reason: String::new(), needs_human_review: false },
        );
        assert!(participating_lenses(&findings, &resolved).is_empty());
    }

    /// #1 핵심 검증: 렌즈 A용 프롬프트에는 렌즈 B의 finding만 등장하고, 렌즈 A 자신의 finding은
    /// (id도 claim도) 전혀 노출되지 않아야 한다 — 자기 finding을 자기가 심사하지 못하게 하는 요구사항.
    #[test]
    fn lens_prompt_excludes_own_findings_includes_others() {
        let findings = vec![finding("f-a1", "lens_a"), finding("f-b1", "lens_b")];
        let resolved = HashMap::new();
        let spec = test_spec();

        let others_for_a: Vec<Finding> = findings.iter().filter(|f| f.lens != "lens_a").cloned().collect();
        let prompt_a = build_moves_prompt_for_lens(&spec, &others_for_a, &resolved, 1, "lens_a");
        assert!(prompt_a.contains("f-b1"), "렌즈 A 프롬프트에 렌즈 B의 finding이 있어야 함");
        assert!(prompt_a.contains("claim-f-b1"));
        assert!(!prompt_a.contains("f-a1"), "렌즈 A 프롬프트에 자기 자신(lens_a)의 finding id가 없어야 함");
        assert!(!prompt_a.contains("claim-f-a1"), "렌즈 A 프롬프트에 자기 자신의 claim이 없어야 함");

        let others_for_b: Vec<Finding> = findings.iter().filter(|f| f.lens != "lens_b").cloned().collect();
        let prompt_b = build_moves_prompt_for_lens(&spec, &others_for_b, &resolved, 1, "lens_b");
        assert!(prompt_b.contains("f-a1"), "렌즈 B 프롬프트에 렌즈 A의 finding이 있어야 함");
        assert!(!prompt_b.contains("f-b1"), "렌즈 B 프롬프트에 자기 자신(lens_b)의 finding id가 없어야 함");
    }

    #[test]
    fn single_lens_has_no_comparison_target() {
        // 렌즈가 1개뿐이면(비교 대상 없음) run_round_call은 critic 호출 자체를 만들지 않는다.
        let findings = vec![finding("f1", "lens_a"), finding("f2", "lens_a")];
        let resolved = HashMap::new();
        assert_eq!(participating_lenses(&findings, &resolved), vec!["lens_a".to_string()]);
        // lenses.len() < 2 분기를 그대로 재현: critic 호출 없이 moves가 빈 채로 진행되어야 함.
        let lenses = participating_lenses(&findings, &resolved);
        assert!(lenses.len() < 2, "렌즈 1개는 비교 대상이 없어 critic 단계가 스킵돼야 함");
    }
}
