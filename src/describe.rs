use crate::input::Input;
use crate::llm::Llm;
use crate::promptctx::shared_context;
use crate::spec::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const DESCRIBE_SYSTEM: &str = "당신은 리서치 문서를 요약하는 애널리스트다. 문서에 없는 내용을 지어내지 않는다. \
반드시 지정된 JSON 스키마로만 응답한다.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Describe {
    pub title: String,
    pub summary: String,
    #[serde(default)]
    pub key_findings: Vec<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    pub can_be_split: String, // yes|no|unknown
    #[serde(default)]
    pub can_be_split_note: String,
}

pub fn run(llm: &Llm, spec: &Spec, input: &Input) -> Result<Describe> {
    let ctx = shared_context(spec, input);
    let task = "# 과제\n아래 리서치 문서의 요약을 작성한다.\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {\"title\":\"50자 이내 한 줄\",\"summary\":\"2~4문장\",\
         \"key_findings\":[\"섹션/영역별 핵심 발견, 항목당 1줄\"],\
         \"labels\":[\"이 문서가 다루는 research-type/영역\"],\
         \"can_be_split\":\"yes|no|unknown\",\"can_be_split_note\":\"근거(예: 섹션 수가 많아 앵글별로 분리 가능한가)\"}\n";
    let v = llm.json_ctx(Some(&ctx), task, Some(DESCRIBE_SYSTEM)).context("describe 실패")?;
    serde_json::from_value(v).context("describe JSON 스키마 불일치")
}

/// 문서에서 "확인 필요"류 마커 스캔. 결정론적(LLM 미사용).
pub fn todo_sections(document: &str) -> Vec<String> {
    let markers = ["[확인필요]", "추후 업데이트", "TODO", "TBD", "재검증 필요"];
    document
        .lines()
        .filter(|l| markers.iter().any(|m| l.contains(m)))
        .map(|l| l.trim().to_string())
        .collect()
}
