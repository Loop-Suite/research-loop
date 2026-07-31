use crate::lens::Finding;
use crate::llm::Llm;
use crate::spec::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// docs/design-spec.md §4: CHALLENGE는 "다른 방법론/다른 소스로 재측정해 수치·주장 불일치를
/// 제기"하는 경우로만 유효 인정(원본 codereview-loop의 "근거·반례·범위 등 반박" 규칙보다 좁힘).
/// 근거 없는 취향 반박("오래된 것 같다")은 SURFACE로 강등하도록 프롬프트에 명시.
pub const DISCOURSE_SYSTEM: &str = "당신은 여러 애널리스트의 finding을 교차검증하는 패널이다. \
내용 없는 동의나 반박은 하지 않는다. AGREE는 새로운 인용/근거가 있을 때만 사용한다. \
CHALLENGE는 '동일 지표를 다른 방법론이나 다른 독립 소스로 재측정해 수치·주장 불일치를 제기'하는 \
경우로만 인정한다 — 근거 없이 '오래된 것 같다', '톤이 별로다' 같은 취향성 반박은 CHALLENGE가 아니라 SURFACE로 제기한다. \
이번 라운드에 CHALLENGE를 최소 1회 포함해야 한다. \
AGREE/CHALLENGE에는 주장 강도에 따른 confidence(high|medium|low)를 반드시 명시한다. \
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

fn build_round_prompt(spec: &Spec, findings: &[Finding], resolved: &HashMap<String, Resolution>, round: usize) -> String {
    format!(
        "# 과제\n라운드 {round} discourse를 수행한다. 봉인되었던 모든 렌즈의 finding을 공개했다.\n\n\
         ## 렌즈 후보(발화자로 사용 가능한 관점)\n{lenses}\n\n\
         ## 전체 findings (미해결 상태만 새로 판정 대상)\n{catalog}\n\n\
         ## 규칙\n\
         - 각 move는 AGREE/CHALLENGE/CONNECT/SURFACE 중 하나, target에 finding id 명시.\n\
         - AGREE: 대상 finding에 없던 새 근거(new_evidence, 독립 소스에서 같은 수치·주장을 재확인)가 있을 때만. confidence 필수.\n\
         - CHALLENGE: 이번 라운드 최소 1회. 반드시 '다른 방법론/다른 소스로 재측정한 불일치'만 인정(취향 반박 금지). confidence 필수.\n\
         - CONNECT: 둘 이상의 finding id를 detail에 명시하며 서로 다른 렌즈의 발견을 연결(예: 재무 발견 ↔ 인센티브 발견).\n\
         - SURFACE: 새 finding을 surfaced 배열에 근거와 함께 추가(기존 lens id 재사용 가능). 근거 없는 반박도 여기로.\n\
         - confidence는 AGREE/CHALLENGE에서만: 주장의 근거 강도가 강하면 high, 보통이면 medium, 약하면 low.\n\
         - resolutions는 UNRESOLVED 또는 이전 라운드 UNCERTAIN이었던 finding만 판정: CONFIRMED|REJECTED|MERGED|UNCERTAIN.\n\
         - 내용 없는 동의/반박은 만들지 말 것.\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"moves\":[{{\"move\":\"AGREE|CHALLENGE|CONNECT|SURFACE\",\"lens\":\"...\",\"target\":\"finding id\",\
         \"detail\":\"...\",\"new_evidence\":\"...\",\"confidence\":\"high|medium|low\"}}],\
         \"resolutions\":[{{\"finding_id\":\"...\",\"status\":\"CONFIRMED|REJECTED|MERGED|UNCERTAIN\",\
         \"merged_into\":\"\",\"reason\":\"...\"}}],\
         \"surfaced\":[{{\"section\":\"...\",\"citation_ref\":\"...\",\"claim\":\"...\",\"evidence\":\"...\",\"impact\":\"...\",\
         \"severity\":\"P0|P1|P2|P3\",\"label\":<허용값 중 하나>,\"confidence\":\"high|medium|low\",\"recommendation\":\"...\",\
         \"citation_status\":\"VERIFIED|UNVERIFIED|STALE|CONTRADICTED\"}}]}}\n",
        round = round,
        lenses = spec.lenses.iter().map(|l| l.id.as_str()).collect::<Vec<_>>().join(", "),
        catalog = findings_catalog(findings, resolved),
    )
}

/// discourse 라운드 반복. 미해결/UNCERTAIN finding이 없어지거나 max_rounds에 도달하면 종료.
/// 매 라운드 CHALLENGE 누락 시 1회 재요청. 로직은 codereview-loop discourse.rs와 동일 —
/// 도메인 차이는 프롬프트(build_round_prompt/DISCOURSE_SYSTEM)의 CHALLENGE 조건 재정의뿐이다.
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
        if !dr.moves.iter().any(|m| m.kind == "CHALLENGE") {
            dr = run_round_call(llm, spec, findings, &resolved, round)
                .context("CHALLENGE 누락 재요청 실패")?;
        }

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
            Resolution { finding_id: f.id.clone(), status, merged_into: String::new(), reason },
        );
    }

    Ok((audit, resolved))
}

fn run_round_call(
    llm: &Llm,
    spec: &Spec,
    findings: &[Finding],
    resolved: &HashMap<String, Resolution>,
    round: usize,
) -> Result<DiscourseRound> {
    let prompt = build_round_prompt(spec, findings, resolved, round);
    let v = llm
        .json(&prompt, Some(DISCOURSE_SYSTEM))
        .with_context(|| format!("discourse 라운드 {round} 실패"))?;
    let dr: DiscourseRound =
        serde_json::from_value(v).with_context(|| format!("discourse 라운드 {round} JSON 스키마 불일치"))?;
    Ok(dr)
}
