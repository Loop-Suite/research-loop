use crate::input::Input;
use crate::llm::Llm;
use crate::promptctx::shared_context;
use crate::spec::{Lens, Spec};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const LENS_SYSTEM: &str = "당신은 시장/경쟁사 리서치 문서를 검증하는 애널리스트 한 명이다. \
근거 없는 의심은 finding이 아니라 unverified로 분리한다. \
문서에 실제로 서술된 주장만 지적하고, 서술되지 않은 내용을 추측해서 만들지 않는다. \
반드시 지정된 JSON 스키마로만 응답한다.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    #[serde(default)]
    pub id: String,
    /// 근거가 위치한 문서 섹션(`## ` 헤딩 텍스트).
    pub section: String,
    /// 근거가 된 인용 번호(citations 목록의 index) 또는 "UNKNOWN".
    #[serde(default = "unknown")]
    pub citation_ref: String,
    pub claim: String,
    pub evidence: String,
    #[serde(default)]
    pub impact: String,
    pub severity: String, // P0-P3
    pub label: String,
    #[serde(default = "unknown")]
    pub confidence: String,
    #[serde(default)]
    pub recommendation: String,
    #[serde(default)]
    pub lens: String,
    /// 이 렌즈의 페르소나 이름(spec에 없으면 빈 문자열).
    #[serde(default)]
    pub reviewer: String,
    /// 인용 신뢰성 판정. LLM이 채운 값을 받지만, checks::verify_citations가 실제 HTTP 재요청 +
    /// 인용 문구 대조로 UNFETCHED|FETCH_FAILED|QUOTE_MATCHED|QUOTE_NOT_FOUND 중 하나로 덮어쓴다(#4).
    /// LLM이 최초에 반환한 값(VERIFIED|UNVERIFIED|STALE|CONTRADICTED 스키마)은 `llm_citation_status`에
    /// 참고용으로만 남는다 — 신뢰의 근거는 이 필드가 아니라 코드가 재산정한 citation_status다.
    #[serde(default = "unverified")]
    pub citation_status: String,
    /// LLM이 최초 판정한 citation_status 원본값(참고용, report에만 advisory로 노출). 코드가 채운다.
    #[serde(default)]
    pub llm_citation_status: String,
}

fn unknown() -> String {
    "UNKNOWN".to_string()
}

fn unverified() -> String {
    "UNVERIFIED".to_string()
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LensOutput {
    #[serde(default)]
    pub findings: Vec<Finding>,
    #[serde(default)]
    pub unverified: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GoodThing {
    pub section: String,
    pub practice: String,
    pub why: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GoodThingsOutput {
    #[serde(default)]
    pub good_things: Vec<GoodThing>,
}

/// 페르소나가 지정된 렌즈는 캐릭터 정체성을 시스템 프롬프트 앞단에 붙인다(동조성 억제).
fn persona_system(lens: &Lens) -> String {
    if lens.persona_name.is_empty() {
        LENS_SYSTEM.to_string()
    } else {
        format!(
            "당신은 \"{}\"이다. {}\n동의를 위한 동의를 하지 않는다 — 이 정체성의 관점에서 판단이 다르면 명확히 다르게 말한다.\n\n{}",
            lens.persona_name, lens.persona_voice, LENS_SYSTEM
        )
    }
}

/// 렌즈 후보(always 제외) 중 research-type/문서 성격에 맞는 것을 LLM으로 선정한다.
pub fn select_lenses(llm: &Llm, spec: &Spec, input: &Input) -> Result<Vec<String>> {
    let optional = spec.optional_lenses();
    if optional.is_empty() {
        return Ok(Vec::new());
    }
    let catalog = optional
        .iter()
        .map(|l| {
            let who = if l.persona_name.is_empty() { l.title.clone() } else { format!("{} ({})", l.title, l.persona_name) };
            format!("- id=\"{}\" | {} — 선정 신호: {}", l.id, who, l.signal)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let ctx = shared_context(spec, input);
    let task = format!(
        "# 과제\n아래 리서치 문서 성격에 맞는 검증 렌즈를 3~5개 고른다(선정 이후 교체 없음).\n\n\
         ## 렌즈 후보\n{catalog}\n\n\
         ## 출력(JSON만)\n{{\"selected\":[\"id\", ...]}}\n",
        catalog = catalog
    );
    let v = llm
        .json_ctx(Some(&ctx), &task, Some("렌즈 선정만 수행하는 리서치 디렉터다. 반드시 JSON 스키마로만 응답한다."))
        .context("렌즈 선정 실패")?;
    let selected: Vec<String> = v
        .get("selected")
        .and_then(|s| s.as_array())
        .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let valid: Vec<String> = selected
        .into_iter()
        .filter(|id| spec.lens_by_id(id).is_some())
        .collect();
    anyhow::ensure!(!valid.is_empty(), "렌즈 선정 결과가 비어있거나 spec에 없는 id뿐");
    Ok(valid)
}

fn build_review_task(spec: &Spec, lens_title: &str, lens_guide: &str) -> String {
    format!(
        "# 과제\n아래 리서치 문서를 \"{lens_title}\" 관점(다른 리뷰어 결과는 참조하지 않음)에서 독립적으로 검증한다.\n\n\
         ## 이 렌즈의 초점\n{lens_guide}\n\n\
         ## 검증 원칙\n\
         - finding마다 근거가 있는 섹션(section)과 인용번호(citation_ref, 문서의 [n] 인용 중 하나 또는 UNKNOWN)를 명시.\n\
         - severity는 P0(치명: 사실오류·수치오염)~P3(사소) 중 하나 — docs/design-spec.md §6 정의를 따른다.\n\
         - citation_status는 VERIFIED(원문 대조로 확인)|UNVERIFIED(대조 안 됨)|STALE(오래된 근거)|CONTRADICTED(다른 근거와 모순) 중 하나.\n\
         - label은 다음 중 하나만: {labels}\n\
         - 근거 없는 의심은 unverified로.\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"findings\":[{{\"section\":\"...\",\"citation_ref\":\"...\",\"claim\":\"...\",\"evidence\":\"...\",\
         \"impact\":\"...\",\"severity\":\"P0|P1|P2|P3\",\"label\":<허용값 중 하나>,\
         \"confidence\":\"high|medium|low\",\"recommendation\":\"...\",\"citation_status\":\"VERIFIED|UNVERIFIED|STALE|CONTRADICTED\"}}],\"unverified\":[\"...\"]}}\n",
        lens_title = lens_title,
        lens_guide = lens_guide,
        labels = spec.labels_prompt(),
    )
}

pub fn review_lens(llm: &Llm, spec: &Spec, input: &Input, lens_id: &str) -> Result<LensOutput> {
    let lens = spec
        .lens_by_id(lens_id)
        .ok_or_else(|| anyhow::anyhow!("spec에 없는 렌즈: {lens_id}"))?;
    let ctx = shared_context(spec, input);
    let task = build_review_task(spec, &lens.title, &lens.guide);
    let system = persona_system(lens);
    let v = llm
        .json_ctx(Some(&ctx), &task, Some(&system))
        .with_context(|| format!("렌즈 리뷰 실패: {lens_id}"))?;
    let mut out: LensOutput =
        serde_json::from_value(v).with_context(|| format!("렌즈 리뷰 JSON 스키마 불일치: {lens_id}"))?;
    let reviewer = if lens.persona_name.is_empty() { lens.title.clone() } else { lens.persona_name.clone() };
    for (i, f) in out.findings.iter_mut().enumerate() {
        f.id = format!("{}-{}", lens_id, i + 1);
        f.lens = lens_id.to_string();
        f.reviewer = reviewer.clone();
        if f.citation_ref.trim().is_empty() {
            f.citation_ref = unknown();
        }
    }
    Ok(out)
}

const GOOD_THINGS_GUIDE: &str = "유지할 가치가 있는 구체적 리서치 관행(예: 실측 데이터 인용, 접근 한계의 정직한 표기)을 찾는다. 근거 없는 칭찬은 만들지 않는다.";

pub fn review_good_things(llm: &Llm, spec: &Spec, input: &Input) -> Result<GoodThingsOutput> {
    let ctx = shared_context(spec, input);
    let task = format!(
        "# 과제\n아래 리서치 문서에서 유지해야 할 좋은 리서치 관행을 찾는다.\n\n\
         ## 이 렌즈의 초점\n{guide}\n\n\
         ## 출력(JSON만, 코드펜스 없이)\n\
         {{\"good_things\":[{{\"section\":\"...\",\"practice\":\"...\",\"why\":\"...\"}}]}}\n\
         근거로 인용할 구체적 사례가 없으면 good_things를 빈 배열로 반환한다.\n",
        guide = GOOD_THINGS_GUIDE,
    );
    let v = llm.json_ctx(Some(&ctx), &task, Some(LENS_SYSTEM)).context("Good Things 렌즈 실패")?;
    let out: GoodThingsOutput =
        serde_json::from_value(v).context("Good Things JSON 스키마 불일치")?;
    Ok(out)
}
