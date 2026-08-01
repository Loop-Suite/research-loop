use crate::lens::Finding;
use crate::llm::Llm;
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
/// #1: 발언(moves)과 판정(resolutions)을 최소 2단계 별도 LLM 호출로 분리한다 —
/// [`run_round_call`] 참조. 완전한 N-way 독립 심의(페르소나별 별도 critic 호출)는 이번 스코프 밖이라
/// "발언 1회 + 판정 1회"까지만 분리했다(이슈 코멘트에 사유 기록).
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
/// 대신 이 값이 "hard evidence"(checks.rs의 결정론적 FAIL)를 절대 뒤집을 수 없도록
/// quantify.rs::verdict()에서 결정론 체크 FAIL을 findings/confidence와 무관한 독립 조건으로
/// 강제한다(quantify.rs의 hard_evidence_fail 우선순위 참고, 테스트로 고정).
fn confidence_weight(c: &str) -> f64 {
    match c {
        "high" => 1.0,
        "low" => 0.3,
        _ => 0.6, // medium 및 미기재
    }
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

/// 1단계(발언) 프롬프트 — moves/surfaced만 요청하고 resolutions는 요청하지 않는다(#1).
fn build_moves_prompt(spec: &Spec, findings: &[Finding], resolved: &HashMap<String, Resolution>, round: usize) -> String {
    format!(
        "# 과제\n라운드 {round} discourse의 1단계(발언)를 수행한다. 봉인되었던 모든 렌즈의 finding을 공개했다. \
         이 단계에서는 판정(CONFIRMED/REJECTED 등)을 내리지 않는다 — 판정은 이어지는 별도 호출이 수행한다.\n\n\
         ## 렌즈 후보(발화자로 사용 가능한 관점)\n{lenses}\n\n\
         ## 전체 findings (미해결 상태만 새로 발언 대상)\n{catalog}\n\n\
         ## 규칙\n\
         - 각 move는 AGREE/CHALLENGE/CONNECT/SURFACE 중 하나, target에 finding id 명시.\n\
         - AGREE: 대상 finding에 없던 새 근거(new_evidence, 독립 소스에서 같은 수치·주장을 재확인)가 있을 때만. confidence 필수.\n\
         - CHALLENGE: 반드시 '다른 방법론/다른 소스로 재측정한 불일치'만 인정(취향 반박 금지). confidence 필수. 없으면 없는 대로 둔다 — 강제 아님.\n\
         - CONNECT: 둘 이상의 finding id를 detail에 명시하며 서로 다른 렌즈의 발견을 연결(예: 재무 발견 ↔ 인센티브 발견).\n\
         - SURFACE: 새 finding을 surfaced 배열에 근거와 함께 추가(기존 lens id 재사용 가능). 근거 없는 반박도 여기로.\n\
         - confidence는 AGREE/CHALLENGE에서만: 주장의 근거 강도가 강하면 high, 보통이면 medium, 약하면 low.\n\
         - 내용 없는 동의/반박은 만들지 말 것.\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"moves\":[{{\"move\":\"AGREE|CHALLENGE|CONNECT|SURFACE\",\"lens\":\"...\",\"target\":\"finding id\",\
         \"detail\":\"...\",\"new_evidence\":\"...\",\"confidence\":\"high|medium|low\"}}],\
         \"surfaced\":[{{\"section\":\"...\",\"citation_ref\":\"...\",\"claim\":\"...\",\"evidence\":\"...\",\"impact\":\"...\",\
         \"severity\":\"P0|P1|P2|P3\",\"label\":<허용값 중 하나>,\"confidence\":\"high|medium|low\",\"recommendation\":\"...\",\
         \"citation_status\":\"VERIFIED|UNVERIFIED|STALE|CONTRADICTED\"}}]}}\n",
        round = round,
        lenses = spec.lenses.iter().map(|l| l.id.as_str()).collect::<Vec<_>>().join(", "),
        catalog = findings_catalog(findings, resolved),
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
/// #1: [`run_round_call`]이 "발언(moves)"과 "판정(resolutions)"을 별도 LLM 호출 2회로 분리해서
/// 수행한다 — 이 함수 자체는 그 결과를 받아 취합하기만 하므로 로직은 이전과 동일하다.
pub fn run(
    llm: &Llm,
    spec: &Spec,
    findings: &mut Vec<Finding>,
    max_rounds: usize,
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

        let mut dr = run_round_call(llm, spec, findings, &resolved, round)?;

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

/// 발언(moves)과 판정(resolutions)을 최소 2단계 별도 LLM 호출로 분리한다(#1) —
/// 1) DISCOURSE_MOVES_SYSTEM으로 moves/surfaced만 생성(반박자 역할).
/// 2) DISCOURSE_ADJUDICATE_SYSTEM으로 1)의 moves만 입력받아 resolutions만 생성(판정자 역할).
/// 완전한 N-way 독립 심의(페르소나별 별도 critic 호출)는 스코프 밖 — "발언"과 "판정"의 분리까지만.
fn run_round_call(
    llm: &Llm,
    spec: &Spec,
    findings: &[Finding],
    resolved: &HashMap<String, Resolution>,
    round: usize,
) -> Result<DiscourseRound> {
    let moves_prompt = build_moves_prompt(spec, findings, resolved, round);
    let mv = llm
        .json(&moves_prompt, Some(DISCOURSE_MOVES_SYSTEM))
        .with_context(|| format!("discourse 라운드 {round} moves 단계 실패"))?;
    let mr: MovesRound =
        serde_json::from_value(mv).with_context(|| format!("discourse 라운드 {round} moves JSON 스키마 불일치"))?;

    let res_prompt = build_resolutions_prompt(findings, resolved, round, &mr.moves);
    let rv = llm
        .json(&res_prompt, Some(DISCOURSE_ADJUDICATE_SYSTEM))
        .with_context(|| format!("discourse 라운드 {round} resolutions 단계 실패"))?;
    let rr: ResolutionsRound =
        serde_json::from_value(rv).with_context(|| format!("discourse 라운드 {round} resolutions JSON 스키마 불일치"))?;

    Ok(DiscourseRound { moves: mr.moves, resolutions: rr.resolutions, surfaced: mr.surfaced })
}
