use crate::input::Input;
use crate::llm::Llm;
use crate::promptctx::shared_context;
use crate::spec::Spec;
use anyhow::{Context, Result};

pub const ASK_SYSTEM: &str = "당신은 리서치 문서에 대한 질문에 답하는 애널리스트다. \
문서·브리프·톤가이드에 근거해서만 답한다. 근거가 없으면 모른다고 답한다.";

pub fn run(llm: &Llm, spec: &Spec, input: &Input, question: &str) -> Result<String> {
    let ctx = shared_context(spec, input);
    let task = format!("# 질문\n{question}\n");
    llm.text_ctx(Some(&ctx), &task, Some(ASK_SYSTEM)).context("ask 실패")
}
