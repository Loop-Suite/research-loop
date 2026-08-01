use crate::input::Input;
use crate::lens::Finding;
use crate::llm::Llm;
use crate::promptctx::shared_context;
use crate::spec::Spec;
use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const REQ_SYSTEM: &str = "당신은 리서치 브리프(다뤄야 할 앵글 목록)의 충족 여부를 문서와 대조해 판정한다. \
근거가 없으면 MET으로 판정하지 않는다. 주어진 REQ-ID 각각에 대해 정확히 하나씩만 응답하고, \
있지도 않은 REQ-ID를 만들어내거나 주어진 항목을 건너뛰지 않는다. 반드시 지정된 JSON 스키마로만 응답한다.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AngleCheck {
    #[serde(default)]
    pub req_id: String,
    pub angle: String,
    pub status: String, // MET|MISSING|AMBIGUOUS|N/A
    pub evidence: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AngleCheckOutput {
    #[serde(default)]
    angles: Vec<AngleCheck>,
}

/// #8: 브리프를 LLM에 통째로 넘기고 "angles" 배열을 그대로 신뢰하던 방식은, 모델이 항목을
/// 아예 누락시켜도 코드가 알아챌 방법이 없었다. 이제 브리프를 프롬프트에 넣기 *전에* 코드가
/// 먼저 결정론적으로 REQ-001, REQ-002... 목록을 만들고, LLM 응답 후 그 ID 집합과 정확히
/// 대조해 누락분을 코드가 강제로 MISSING 처리한다.
///
/// 파싱 규칙: 빈 줄은 건너뛰고, 각 줄 앞의 번호매김(`1.` `1)` `(1)`)이나 글머리기호(`-` `*` `•`)를
/// 벗겨낸 나머지를 항목 텍스트로 취급한다. 마커가 없는 일반 줄도 그 자체로 한 항목이 된다
/// (완전한 마크다운 리스트 AST 파서는 아니지만, 줄바꿈 기준 결정론적 분해로 "LLM이 통째로 판단"하는
/// 문제는 해소한다).
fn parse_requirements(brief: &str) -> Vec<(String, String)> {
    let marker_re = Regex::new(r"^\s*(?:[-*•]|\(?\d+[.)])\s+").expect("요구사항 마커 정규식 컴파일 실패");
    let mut items: Vec<String> = Vec::new();
    for line in brief.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let text = marker_re.replace(trimmed, "").trim().to_string();
        if text.is_empty() {
            continue;
        }
        items.push(text);
    }
    items
        .into_iter()
        .enumerate()
        .map(|(i, text)| (format!("REQ-{:03}", i + 1), text))
        .collect()
}

/// requirements(브리프) 미제공 시 None 반환(검증 대상 없음, N/A 나열하지 않음).
pub fn verify(llm: &Llm, spec: &Spec, input: &Input, confirmed: &[&Finding]) -> Result<Option<Vec<AngleCheck>>> {
    let brief = match &input.requirements {
        None => return Ok(None),
        Some(b) => b,
    };
    let reqs = parse_requirements(brief);
    if reqs.is_empty() {
        return Ok(Some(Vec::new()));
    }

    let req_list = reqs.iter().map(|(id, text)| format!("- {id}: {text}")).collect::<Vec<_>>().join("\n");
    let findings_summary = confirmed
        .iter()
        .map(|f| format!("- [{}] {} — {}", f.severity, f.section, f.claim))
        .collect::<Vec<_>>()
        .join("\n");
    let ctx = shared_context(spec, input);
    let task = format!(
        "# 과제\n아래 결정론적으로 번호매김된 요구사항(REQ-ID) 각각을 문서와 대조해 판정한다. \
         반드시 주어진 모든 REQ-ID에 대해 정확히 하나씩 항목을 반환한다(누락·추가 금지).\n\n\
         ## 요구사항 목록(REQ-ID: 브리프 원문)\n{req_list}\n\n\
         ## 확정된 findings(참고용, 앵글 미충족의 근거가 될 수 있음)\n{fs}\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"angles\":[{{\"req_id\":\"REQ-001\",\"angle\":\"요구사항 목록의 해당 원문 그대로\",\
         \"status\":\"MET|MISSING|AMBIGUOUS|N/A\",\"evidence\":\"섹션 근거 또는 누락/모호 사유\"}}]}}\n",
        req_list = req_list,
        fs = if findings_summary.is_empty() { "(없음)".to_string() } else { findings_summary },
    );
    let v = llm.json_ctx(Some(&ctx), &task, Some(REQ_SYSTEM)).context("앵글 커버리지 검증 실패")?;
    let out: AngleCheckOutput = serde_json::from_value(v).context("앵글 커버리지 JSON 스키마 불일치")?;

    // 결정론적 대조: LLM이 낸 req_id 중 우리가 준 목록에 있는 것만 신뢰하고, 누락된 REQ-ID는
    // 코드가 강제로 MISSING 처리한다 — 모델이 조용히 항목을 빠뜨리는 것을 방지(#8).
    let mut by_id: HashMap<String, AngleCheck> = out
        .angles
        .into_iter()
        .filter(|a| !a.req_id.trim().is_empty())
        .map(|a| (a.req_id.clone(), a))
        .collect();
    let mut result = Vec::with_capacity(reqs.len());
    for (id, text) in &reqs {
        match by_id.remove(id) {
            Some(mut a) => {
                a.req_id = id.clone();
                if a.angle.trim().is_empty() {
                    a.angle = text.clone();
                }
                result.push(a);
            }
            None => result.push(AngleCheck {
                req_id: id.clone(),
                angle: text.clone(),
                status: "MISSING".to_string(),
                evidence: "LLM 출력에 이 REQ-ID가 없음 — 코드가 결정론적으로 MISSING 처리(요구사항 누락을 모델이 조용히 빠뜨리는 것 방지)".to_string(),
            }),
        }
    }
    Ok(Some(result))
}

/// MISSING/AMBIGUOUS 앵글만 추출 — report.rs의 coverage_gaps 섹션에 그대로 쓰인다.
pub fn coverage_gaps(angles: &Option<Vec<AngleCheck>>) -> Vec<String> {
    match angles {
        None => Vec::new(),
        Some(list) => list
            .iter()
            .filter(|a| a.status == "MISSING" || a.status == "AMBIGUOUS")
            .map(|a| format!("{} {} ({})", a.req_id, a.angle, a.status))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_numbered_list() {
        let brief = "1. 가격 정책\n2) 경쟁사 비교\n(3) 시장 규모\n";
        let reqs = parse_requirements(brief);
        assert_eq!(reqs.len(), 3);
        assert_eq!(reqs[0], ("REQ-001".to_string(), "가격 정책".to_string()));
        assert_eq!(reqs[1], ("REQ-002".to_string(), "경쟁사 비교".to_string()));
        assert_eq!(reqs[2], ("REQ-003".to_string(), "시장 규모".to_string()));
    }

    #[test]
    fn parses_bullets_and_plain_lines() {
        let brief = "- 가격 정책\n* 경쟁사 비교\n• 시장 규모\n그냥 줄\n\n\n빈 줄 건너뜀";
        let reqs = parse_requirements(brief);
        assert_eq!(reqs.len(), 5);
        assert_eq!(reqs[3].1, "그냥 줄");
    }

    #[test]
    fn empty_brief_yields_no_requirements() {
        assert!(parse_requirements("   \n\n  ").is_empty());
    }
}
