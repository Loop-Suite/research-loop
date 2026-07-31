use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 리서치 렌즈(페르소나 7종 중 research-type 성격에 맞게 선택되는 항목).
/// codereview-loop의 Lens와 필드가 동일하다 — 도메인은 프롬프트(guide/persona_voice)로만 구분한다.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Lens {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub guide: String,
    /// true면 렌즈 선정 단계에서 매번 강제 포함.
    #[serde(default)]
    pub always: bool,
    /// 이 렌즈를 고르게 하는 신호(선택 프롬프트에 그대로 삽입).
    #[serde(default)]
    pub signal: String,
    /// 캐릭터화 페르소나 이름(비우면 무페르소나). 동조성(sycophancy) 억제 목적.
    #[serde(default)]
    pub persona_name: String,
    /// 페르소나의 관점/원칙 한 줄.
    #[serde(default)]
    pub persona_voice: String,
    /// 표시용 문자열(예: 1/2). 선택 로직에는 관여하지 않는다 — docs/design-spec.md §1 가정 참조.
    #[serde(default)]
    pub tier: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Spec {
    pub name: String,
    /// 리서치 대상·맥락(예: "국내 카페 POS 경쟁사"). 프롬프트에 그대로 삽입.
    #[serde(default)]
    pub context: String,
    pub lenses: Vec<Lens>,
    /// findings에 허용되는 label 목록.
    pub labels: Vec<String>,
    /// 리서치 대상 기업이 직접 발행한 도메인 목록(자사 발행 콘텐츠 판별용, source_diversity_check).
    /// 예: ["tossplace.com", "payhere.in"]. 비어있으면 해당 체크는 NOT_CONFIGURED.
    #[serde(default)]
    pub subject_owned_domains: Vec<String>,
    /// 인용 근거의 "오래됨" 판정 임계값(년). 0이면 미설정(staleness_flag 비활성).
    #[serde(default)]
    pub staleness_threshold_years: u32,
    /// 결정론 검사 활성화 여부와 무관하게 항상 실행되는 checks.rs 산출 항목 id 목록.
    /// 비어있으면 checks.rs가 계산 가능한 항목 전부 실행.
    #[serde(default)]
    pub enabled_checks: Vec<String>,
}

impl Spec {
    pub fn load(path: &Path) -> Result<Spec> {
        let s = std::fs::read_to_string(path)
            .with_context(|| format!("스펙 파일 읽기 실패: {}", path.display()))?;
        let spec: Spec = toml::from_str(&s)
            .with_context(|| format!("스펙 TOML 파싱 실패: {}", path.display()))?;
        anyhow::ensure!(!spec.lenses.is_empty(), "lenses 비어 있음");
        anyhow::ensure!(!spec.labels.is_empty(), "labels 비어 있음");
        Ok(spec)
    }

    pub fn lens_by_id(&self, id: &str) -> Option<&Lens> {
        self.lenses.iter().find(|l| l.id == id)
    }

    pub fn always_lenses(&self) -> Vec<&Lens> {
        self.lenses.iter().filter(|l| l.always).collect()
    }

    pub fn optional_lenses(&self) -> Vec<&Lens> {
        self.lenses.iter().filter(|l| !l.always).collect()
    }

    pub fn labels_prompt(&self) -> String {
        self.labels
            .iter()
            .map(|l| format!("\"{l}\""))
            .collect::<Vec<_>>()
            .join(", ")
    }

    pub fn check_enabled(&self, id: &str) -> bool {
        self.enabled_checks.is_empty() || self.enabled_checks.iter().any(|c| c == id)
    }
}
