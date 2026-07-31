use crate::input::Input;
use crate::lens::Finding;
use crate::llm::Llm;
use crate::promptctx::shared_context;
use crate::spec::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// codereview-loop의 FIXED/STILL_OPEN/UNKNOWN 3상태에 REVERSED를 추가한다
/// (docs/design-spec.md §0 — 티오더-KT 사례처럼 "이전 결론이 최신 근거로 뒤집힌 경우"를
/// STILL_OPEN과 구분하기 위한 리서치 도메인 확장. 원본에 없던 4번째 값).
pub const FIXCHECK_SYSTEM: &str = "당신은 이전 라운드에서 확정된 finding이 이번 문서에서 실제로 반영됐는지 판정한다. \
근거 없이 FIXED로 판정하지 않는다. 단순히 갱신된 게 아니라 '최신 근거로 이전 결론 자체가 뒤집힌' 경우는 \
FIXED가 아니라 REVERSED로 판정한다(예: 이전엔 확정 사실로 서술했는데 최신 근거가 정반대를 가리키는 경우). \
확인 불가하면 UNKNOWN. 반드시 지정된 JSON 스키마로만 응답한다.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixStatus {
    pub finding_id: String,
    pub status: String, // FIXED|STILL_OPEN|UNKNOWN|REVERSED
    pub evidence: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FixCheckOutput {
    #[serde(default)]
    results: Vec<FixStatus>,
}

/// prior_confirmed 비어있으면 빈 결과(라운드 1이거나 이전에 확정 finding 없음).
pub fn run(llm: &Llm, spec: &Spec, input: &Input, prior_confirmed: &[Finding]) -> Result<Vec<FixStatus>> {
    if prior_confirmed.is_empty() {
        return Ok(Vec::new());
    }
    let list = prior_confirmed
        .iter()
        .map(|f| format!("- id={} | {} | {}\n  근거: {}", f.id, f.section, f.claim, f.evidence))
        .collect::<Vec<_>>()
        .join("\n");
    let ctx = shared_context(spec, input);
    let task = format!(
        "# 과제\n이전 라운드에서 확정된 아래 finding들이 이번 문서에서 반영/번복됐는지 판정한다.\n\n\
         ## 이전 라운드 확정 findings\n{list}\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"results\":[{{\"finding_id\":\"...\",\"status\":\"FIXED|STILL_OPEN|UNKNOWN|REVERSED\",\"evidence\":\"...\"}}]}}\n",
        list = list
    );
    let v = llm.json_ctx(Some(&ctx), &task, Some(FIXCHECK_SYSTEM)).context("fix check 실패")?;
    let out: FixCheckOutput = serde_json::from_value(v).context("fix check JSON 스키마 불일치")?;
    Ok(out.results)
}
