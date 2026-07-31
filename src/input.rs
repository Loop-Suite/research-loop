use anyhow::{Context, Result};
use regex::Regex;
use std::path::Path;

/// 문서 내 인용(마크다운 링크) 1건.
#[derive(Debug, Clone)]
pub struct Citation {
    pub index: usize,
    pub text: String,
    pub url: String,
}

/// 정규화된 입력. 없는 정보는 None으로 남기고 UNKNOWN 취급은 호출부(report)에서 표시한다.
pub struct Input {
    pub document: String,
    /// `## ` 헤딩 목록(codereview-loop의 changed_files 대응 — 섹션 단위 근거 참조에 쓰임).
    pub sections: Vec<String>,
    pub word_count: usize,
    pub citations: Vec<Citation>,
    /// 리서치 브리프: 반드시 다뤄야 할 앵글 목록(codereview-loop의 requirements 대응).
    pub requirements: Option<String>,
    /// 톤/포맷 가이드(codereview-loop의 conventions 대응).
    pub conventions: Option<String>,
    /// checks.rs 결과. None이면 review 서브커맨드 실행 중 자체 계산.
    pub deterministic_results: Option<serde_json::Value>,
}

fn read_opt(p: &Option<std::path::PathBuf>) -> Result<Option<String>> {
    match p {
        None => Ok(None),
        Some(path) => {
            let s = std::fs::read_to_string(path)
                .with_context(|| format!("파일 읽기 실패: {}", path.display()))?;
            Ok(Some(s))
        }
    }
}

fn extract_sections(doc: &str) -> Vec<String> {
    doc.lines()
        .filter_map(|l| l.strip_prefix("## ").map(|s| s.trim().to_string()))
        .collect()
}

/// 마크다운 링크 `[text](url)` 전부 추출. http(s) 스킴만 인용으로 취급(내부 앵커 `#` 제외).
fn extract_citations(doc: &str) -> Vec<Citation> {
    let re = Regex::new(r"\[([^\]]*)\]\((https?://[^)\s]+)\)").expect("citation regex 컴파일 실패");
    re.captures_iter(doc)
        .enumerate()
        .map(|(i, c)| Citation {
            index: i + 1,
            text: c.get(1).map(|m| m.as_str().to_string()).unwrap_or_default(),
            url: c.get(2).map(|m| m.as_str().to_string()).unwrap_or_default(),
        })
        .collect()
}

pub fn normalize(
    document_path: &Path,
    requirements_path: &Option<std::path::PathBuf>,
    conventions_path: &Option<std::path::PathBuf>,
    deterministic_results_path: &Option<std::path::PathBuf>,
) -> Result<Input> {
    let document = std::fs::read_to_string(document_path)
        .with_context(|| format!("리서치 문서 읽기 실패: {}", document_path.display()))?;
    anyhow::ensure!(!document.trim().is_empty(), "리서치 문서가 비어 있음");

    let sections = extract_sections(&document);
    let citations = extract_citations(&document);
    let word_count = document.split_whitespace().count();

    let requirements = read_opt(requirements_path)?;
    let conventions = read_opt(conventions_path)?;
    let deterministic_results = match deterministic_results_path {
        None => None,
        Some(p) => {
            let s = std::fs::read_to_string(p)
                .with_context(|| format!("결정론 결과 파일 읽기 실패: {}", p.display()))?;
            Some(
                serde_json::from_str(&s)
                    .with_context(|| format!("결정론 결과 JSON 파싱 실패: {}", p.display()))?,
            )
        }
    };

    Ok(Input {
        document,
        sections,
        word_count,
        citations,
        requirements,
        conventions,
        deterministic_results,
    })
}
