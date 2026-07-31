use crate::input::Input;
use crate::llm::Llm;
use crate::promptctx::shared_context;
use crate::spec::Spec;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const IMPROVE_SYSTEM: &str = "당신은 리서치 문서에 구체적인 개정안을 제시하는 애널리스트다. \
문서에 실제로 서술된 내용에 대해서만 제안한다. 근거 없이 새 사실을 지어내지 않는다 — \
개정 제안은 '이 부분을 이렇게 추가/재조사하라'는 지시이지, 확인 안 된 수치를 임의로 채우는 것이 아니다. \
반드시 지정된 JSON 스키마로만 응답한다.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suggestion {
    pub relevant_section: String,
    pub existing_text: String,
    pub suggestion_content: String,
    pub revised_text: String,
    pub one_sentence_summary: String,
    pub label: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ImproveOutput {
    #[serde(default)]
    suggestions: Vec<Suggestion>,
}

pub fn run(llm: &Llm, spec: &Spec, input: &Input) -> Result<Vec<Suggestion>> {
    let ctx = shared_context(spec, input);
    let task = format!(
        "# 과제\n이 리서치 문서에 대해 구체적인 개정안(추가 조사 반영/정정)을 제시한다.\n\n\
         ## 규칙\n\
         - existing_text/revised_text는 실제 문서에 있는 문장을 그대로 인용/수정.\n\
         - 확인 안 된 새 수치를 지어내지 말 것 — '이 부분을 재조사하라'는 지시로 대신할 것.\n\
         - one_sentence_summary는 6단어 이내.\n\
         - label은 다음 중 하나만: {labels}\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"suggestions\":[{{\"relevant_section\":\"...\",\"existing_text\":\"...\",\
         \"suggestion_content\":\"...\",\"revised_text\":\"...\",\"one_sentence_summary\":\"...\",\
         \"label\":<허용값 중 하나>}}]}}\n",
        labels = spec.labels_prompt(),
    );
    let v = llm.json_ctx(Some(&ctx), &task, Some(IMPROVE_SYSTEM)).context("improve 실패")?;
    let out: ImproveOutput = serde_json::from_value(v).context("improve JSON 스키마 불일치")?;
    Ok(out.suggestions)
}
