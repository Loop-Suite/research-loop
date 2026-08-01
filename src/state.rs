use crate::discourse::Resolution;
use crate::lens::Finding;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// 재현·감사용 fingerprint. 암호학적 해시가 아니다 — `std::hash::DefaultHasher`(SipHash13,
/// 고정 키 `(0,0)`이라 프로세스/머신에 무관하게 결정론적)를 그대로 쓴 64비트 값이다.
/// 목적은 "다음 라운드/재실행 때 입력이 바뀌었는지" 빠르게 비교하는 것이지 보안 무결성 검증이
/// 아니다(#9 — RunManifest 개념 도입, 완전한 SHA-256급 해시는 스코프 밖).
fn fingerprint(s: &str) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    format!("{:016x}", h.finish())
}

pub fn fingerprint_str(s: &str) -> String {
    fingerprint(s)
}

/// UTC 유닉스 타임스탬프(초) 문자열. chrono 의존성을 새로 들이지 않기 위해 캘린더 포맷팅 없이
/// epoch seconds만 기록한다 — 타임존 모호성 없이 항상 UTC 기준.
pub fn unix_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

/// 라운드 종료 시점의 findings·판정 스냅샷. 다음 라운드(--prior)가 이어받는다.
///
/// #9: round/findings/resolved 3필드뿐이던 것에 재현·감사용 필드를 추가했다(RunManifest 개념).
/// 모두 `#[serde(default)]`라 이 필드들이 없는 과거 state.json(--prior)도 그대로 로드된다.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub round: usize,
    pub findings: Vec<Finding>,
    pub resolved: HashMap<String, Resolution>,
    /// 입력 문서 원문의 fingerprint(비암호학적, [`fingerprint`] 참고).
    #[serde(default)]
    pub input_hash: String,
    /// spec(TOML) 전체 직렬화의 fingerprint.
    #[serde(default)]
    pub spec_hash: String,
    /// 실행에 쓰인 모델 id(미지정이면 백엔드 기본값이라는 뜻 — 빈 문자열로 남음).
    #[serde(default)]
    pub model_id: String,
    /// "claude-cli" | "openrouter".
    #[serde(default)]
    pub provider: String,
    /// 라운드 시작 시각(UTC unix epoch seconds, 문자열).
    #[serde(default)]
    pub started_at: String,
    /// 라운드 완료 시각(UTC unix epoch seconds, 문자열).
    #[serde(default)]
    pub completed_at: String,
    /// llm.usage().cost_usd 누적치(claude CLI가 값을 제공하지 않으면 0.0 — OpenRouter 응답엔 비용 필드 없음).
    #[serde(default)]
    pub cost_usd: f64,
    /// 프롬프트 스키마 버전. 프롬프트 구조(JSON 스키마 등)가 바뀔 때 수동으로 올린다 —
    /// 과거 라운드와 결과를 비교할 때 "프롬프트 자체가 달라졌는지"를 구분하기 위함.
    #[serde(default)]
    pub prompt_version: String,
}

pub fn write(out_dir: &Path, state: &State) -> Result<PathBuf> {
    let path = out_dir.join("state.json");
    std::fs::write(&path, serde_json::to_string_pretty(state)?)
        .with_context(|| format!("{} 쓰기 실패", path.display()))?;
    Ok(path)
}

pub fn load(dir: &Path) -> Result<State> {
    let path = if dir.is_dir() { dir.join("state.json") } else { dir.to_path_buf() };
    let s = std::fs::read_to_string(&path).with_context(|| format!("{} 읽기 실패 (--prior는 이전 --out 디렉터리)", path.display()))?;
    serde_json::from_str(&s).with_context(|| format!("{} 파싱 실패", path.display()))
}
