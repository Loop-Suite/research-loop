use crate::input::Input;
use crate::lens::Finding;
use crate::llm::Llm;
use crate::promptctx::shared_context;
use crate::spec::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const REQ_SYSTEM: &str = "당신은 리서치 브리프(다뤄야 할 앵글 목록)의 충족 여부를 문서와 대조해 판정한다. \
근거가 없으면 MET으로 판정하지 않는다. 반드시 지정된 JSON 스키마로만 응답한다.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AngleCheck {
    pub angle: String,
    pub status: String, // MET|MISSING|AMBIGUOUS|N/A
    pub evidence: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AngleCheckOutput {
    #[serde(default)]
    angles: Vec<AngleCheck>,
}

/// requirements(브리프) 미제공 시 None 반환(검증 대상 없음, N/A 나열하지 않음).
pub fn verify(llm: &Llm, spec: &Spec, input: &Input, confirmed: &[&Finding]) -> Result<Option<Vec<AngleCheck>>> {
    if input.requirements.is_none() {
        return Ok(None);
    }
    let findings_summary = confirmed
        .iter()
        .map(|f| format!("- [{}] {} — {}", f.severity, f.section, f.claim))
        .collect::<Vec<_>>()
        .join("\n");
    let ctx = shared_context(spec, input);
    let task = format!(
        "# 과제\n리서치 브리프의 각 앵글을 문서와 대조해 판정한다.\n\n\
         ## 확정된 findings(참고용, 앵글 미충족의 근거가 될 수 있음)\n{fs}\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"angles\":[{{\"angle\":\"브리프 원문 그대로\",\"status\":\"MET|MISSING|AMBIGUOUS|N/A\",\
         \"evidence\":\"섹션 근거 또는 누락/모호 사유\"}}]}}\n",
        fs = if findings_summary.is_empty() { "(없음)".to_string() } else { findings_summary },
    );
    let v = llm.json_ctx(Some(&ctx), &task, Some(REQ_SYSTEM)).context("앵글 커버리지 검증 실패")?;
    let out: AngleCheckOutput =
        serde_json::from_value(v).context("앵글 커버리지 JSON 스키마 불일치")?;
    Ok(Some(out.angles))
}

/// MISSING/AMBIGUOUS 앵글만 추출 — report.rs의 coverage_gaps 섹션에 그대로 쓰인다.
pub fn coverage_gaps(angles: &Option<Vec<AngleCheck>>) -> Vec<String> {
    match angles {
        None => Vec::new(),
        Some(list) => list
            .iter()
            .filter(|a| a.status == "MISSING" || a.status == "AMBIGUOUS")
            .map(|a| format!("{} ({})", a.angle, a.status))
            .collect(),
    }
}
