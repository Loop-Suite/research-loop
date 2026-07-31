use crate::input::Input;
use crate::spec::Spec;

/// 모든 LLM 호출이 공유하는 컨텍스트 블록(리서치 맥락·톤가이드·브리프·문서 원문).
pub fn shared_context(spec: &Spec, input: &Input) -> String {
    let mut c = String::new();
    c.push_str(&format!("## 리서치 대상/맥락\n{}\n\n", spec.context));
    if let Some(conv) = &input.conventions {
        c.push_str(&format!("## 톤/포맷 가이드(원문, 명시적 브리프 다음으로 우선)\n{}\n\n", conv));
    }
    if let Some(req) = &input.requirements {
        c.push_str(&format!("## 리서치 브리프(다뤄야 할 앵글)\n{}\n\n", req));
    }
    c.push_str(&format!(
        "## 문서 섹션 ({}개, {}단어, 인용 {}건)\n{}\n\n",
        input.sections.len(),
        input.word_count,
        input.citations.len(),
        input.sections.join(", ")
    ));
    if !input.citations.is_empty() {
        c.push_str("## 인용 목록 (finding의 citation_ref는 이 번호를 참조)\n");
        for cit in &input.citations {
            c.push_str(&format!("[{}] {} — {}\n", cit.index, cit.text, cit.url));
        }
        c.push('\n');
    }
    c.push_str(&format!("## 리서치 문서 원문\n```markdown\n{}\n```\n\n", input.document));
    c
}
