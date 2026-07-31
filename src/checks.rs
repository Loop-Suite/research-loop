//! 결정론적(LLM 미사용) 검사. codereview-loop의 policy.rs+semgrep.rs를 통합한 것 —
//! 리서치 도메인에는 "외부 결정론 도구가 채워주는 자동 스캐너"(semgrep 대응물)가 없어서
//! 굳이 두 모듈로 나눌 이유가 없다는 판단(docs/design-spec.md §3 "semgrep 대응 구조적 차이" 참조).
//! docs/research-and-evidence-survey §2에서 관찰한 6개 실패모드에 1:1 대응한다.

use crate::input::Input;
use crate::spec::Spec;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    NotApplicable,
    NotConfigured,
}

impl CheckStatus {
    pub fn label(&self) -> &'static str {
        match self {
            CheckStatus::Pass => "PASS",
            CheckStatus::Warn => "WARN",
            CheckStatus::Fail => "FAIL",
            CheckStatus::NotApplicable => "N/A",
            CheckStatus::NotConfigured => "NOT_CONFIGURED",
        }
    }
}

pub struct CheckResult {
    pub id: String,
    pub title: String,
    pub status: CheckStatus,
    pub evidence: String,
}

/// 실패모드1: 일반. 문장(대략 "다."/"음."/"됨." 종결) 대비 인용 수 비율.
/// 가정: 문장 경계 탐지는 정확한 형태소 분석이 아니라 종결어미 휴리스틱이다 — 근사치로만 사용(불확실).
fn citation_density_check(input: &Input) -> CheckResult {
    let sentence_endings = ["다.", "음.", "됨.", "함."];
    let approx_sentences: usize = sentence_endings
        .iter()
        .map(|e| input.document.matches(e).count())
        .sum();
    let citations = input.citations.len();
    if approx_sentences == 0 {
        return CheckResult {
            id: "citation_density".into(),
            title: "주장 대비 인용 밀도".into(),
            status: CheckStatus::NotApplicable,
            evidence: "문장 종결 탐지 실패(휴리스틱 한계)".into(),
        };
    }
    let ratio = citations as f64 / approx_sentences as f64;
    let status = if ratio >= 0.05 { CheckStatus::Pass } else { CheckStatus::Warn };
    CheckResult {
        id: "citation_density".into(),
        title: "주장 대비 인용 밀도".into(),
        status,
        evidence: format!("근사 문장수 {approx_sentences}, 인용 {citations}건 (비율 {ratio:.3}, 휴리스틱 근사치)"),
    }
}

/// 실패모드4 대응 축소판: "자사발행 콘텐츠가 검색결과 장악" — 인용 도메인 중
/// spec.subject_owned_domains 비중을 계산.
fn source_diversity_check(spec: &Spec, input: &Input) -> CheckResult {
    if spec.subject_owned_domains.is_empty() {
        return CheckResult {
            id: "source_diversity".into(),
            title: "출처 다양성(자사발행 비중)".into(),
            status: CheckStatus::NotConfigured,
            evidence: "spec.subject_owned_domains 미설정".into(),
        };
    }
    if input.citations.is_empty() {
        return CheckResult {
            id: "source_diversity".into(),
            title: "출처 다양성(자사발행 비중)".into(),
            status: CheckStatus::NotApplicable,
            evidence: "인용 없음".into(),
        };
    }
    let owned = input
        .citations
        .iter()
        .filter(|c| spec.subject_owned_domains.iter().any(|d| c.url.contains(d.as_str())))
        .count();
    let ratio = owned as f64 / input.citations.len() as f64;
    let status = if ratio <= 0.4 { CheckStatus::Pass } else { CheckStatus::Warn };
    CheckResult {
        id: "source_diversity".into(),
        title: "출처 다양성(자사발행 비중)".into(),
        status,
        evidence: format!("전체 인용 {}건 중 자사발행 도메인 {}건 ({:.0}%)", input.citations.len(), owned, ratio * 100.0),
    }
}

/// 실패모드5: "동일 지표의 회차별 수치 불일치". 국내 통화 표현(억/조/원, %) 앞의 2~4어절을
/// 키로 묶어 동일 문구에 서로 다른 숫자가 붙었는지 탐지.
/// 가정: 형태소 분석이 아닌 어절 윈도 매칭이라 오탐/누락 가능 — WARN일 뿐 FAIL 아님(불확실).
fn numeric_consistency_check(input: &Input) -> CheckResult {
    let re = Regex::new(r"([\p{Hangul}A-Za-z]{2,6}(?:\s+[\p{Hangul}A-Za-z]{1,6}){0,2})\s*([0-9][0-9,]*(?:\.[0-9]+)?)\s*(억원|조원|억|%|명|개)")
        .expect("numeric regex 컴파일 실패");
    let mut seen: HashMap<String, Vec<String>> = HashMap::new();
    for cap in re.captures_iter(&input.document) {
        let phrase = cap.get(1).map(|m| m.as_str().trim().to_string()).unwrap_or_default();
        let value = format!("{}{}", cap.get(2).map(|m| m.as_str()).unwrap_or(""), cap.get(3).map(|m| m.as_str()).unwrap_or(""));
        if phrase.chars().count() < 2 {
            continue;
        }
        seen.entry(phrase).or_default().push(value);
    }
    let conflicts: Vec<String> = seen
        .into_iter()
        .filter_map(|(phrase, values)| {
            let unique: std::collections::HashSet<&String> = values.iter().collect();
            if unique.len() > 1 {
                Some(format!("\"{}\": {}", phrase, values.join(" vs ")))
            } else {
                None
            }
        })
        .collect();
    if conflicts.is_empty() {
        CheckResult {
            id: "numeric_consistency".into(),
            title: "수치 일관성(동일 문구 반복 수치 대조)".into(),
            status: CheckStatus::Pass,
            evidence: "동일 문구에 서로 다른 수치가 붙은 사례 없음(휴리스틱 탐지 기준)".into(),
        }
    } else {
        CheckResult {
            id: "numeric_consistency".into(),
            title: "수치 일관성(동일 문구 반복 수치 대조)".into(),
            status: CheckStatus::Warn,
            evidence: format!("잠재적 불일치 {}건 — {}", conflicts.len(), conflicts.join(" | ")),
        }
    }
}

/// 실패모드6: "폐쇄형 플랫폼 접근 불가". "확인 안 됨"류 정직 표기가 문서에 최소 1회 있는지만 확인 —
/// 없다고 반드시 문제는 아니지만(모든 리서치가 접근제약을 겪는 건 아님), 있으면 그 자체로 긍정 신호.
fn access_limitation_disclosure_check(input: &Input) -> CheckResult {
    let markers = ["확인 안 됨", "접근 불가", "단정할 근거 없음", "확인 안됨", "미확인"];
    let hits: usize = markers.iter().map(|m| input.document.matches(m).count()).sum();
    CheckResult {
        id: "access_limitation_disclosure".into(),
        title: "접근 한계 정직 표기".into(),
        status: if hits > 0 { CheckStatus::Pass } else { CheckStatus::NotApplicable },
        evidence: format!("정직 표기 문구 {hits}건 발견(해당 없으면 리서치 범위 내 접근 제약이 없었다는 뜻일 수도 있음)"),
    }
}

/// 실패모드3: "인센티브 리뷰로 인한 신뢰도 오염". 인센티브 관련 키워드가 문서에 등장하면
/// PASS/FAIL이 아니라 정보성 WARN으로 표시 — 실제 표기 적절성은 discourse(citation_status)가 판단.
fn incentive_disclosure_scan(input: &Input) -> CheckResult {
    let markers = ["리뷰 이벤트", "협찬", "제휴 리뷰", "보상 프로그램", "인센티브", "현금 보상"];
    let hits: Vec<&str> = markers.iter().filter(|m| input.document.contains(*m)).copied().collect();
    if hits.is_empty() {
        CheckResult {
            id: "incentive_disclosure".into(),
            title: "인센티브 리뷰 언급 스캔".into(),
            status: CheckStatus::Pass,
            evidence: "인센티브 관련 키워드 없음".into(),
        }
    } else {
        CheckResult {
            id: "incentive_disclosure".into(),
            title: "인센티브 리뷰 언급 스캔".into(),
            status: CheckStatus::Warn,
            evidence: format!("키워드 발견: {} — 인용된 후기가 이 인센티브의 영향을 받았는지 discourse 라운드에서 재확인 필요", hits.join(", ")),
        }
    }
}

/// 실패모드7: "이전 결론이 최신 정보로 뒤집힘"에 대응하는 최신성 체크.
/// spec.staleness_threshold_years=0이면 비활성. 문서에서 4자리 연도를 모두 추출해
/// as_of_year 대비 임계값을 초과하는 연도가 있으면 WARN(오래된 근거 존재 가능성).
fn staleness_flag(spec: &Spec, input: &Input, as_of_year: u32) -> CheckResult {
    if spec.staleness_threshold_years == 0 {
        return CheckResult {
            id: "staleness".into(),
            title: "인용 최신성".into(),
            status: CheckStatus::NotConfigured,
            evidence: "spec.staleness_threshold_years 미설정".into(),
        };
    }
    let re = Regex::new(r"(19|20)\d{2}").expect("year regex 컴파일 실패");
    let old_years: std::collections::HashSet<u32> = re
        .find_iter(&input.document)
        .filter_map(|m| m.as_str().parse::<u32>().ok())
        .filter(|y| as_of_year.saturating_sub(*y) > spec.staleness_threshold_years && *y <= as_of_year)
        .collect();
    if old_years.is_empty() {
        CheckResult {
            id: "staleness".into(),
            title: "인용 최신성".into(),
            status: CheckStatus::Pass,
            evidence: format!("임계값({}년) 초과 연도 없음", spec.staleness_threshold_years),
        }
    } else {
        let mut ys: Vec<u32> = old_years.into_iter().collect();
        ys.sort();
        CheckResult {
            id: "staleness".into(),
            title: "인용 최신성".into(),
            status: CheckStatus::Warn,
            evidence: format!("임계값({}년) 초과 연도 등장: {:?} — 최신 근거로 재검증 권장", spec.staleness_threshold_years, ys),
        }
    }
}

/// 실측 데드링크 확인. ureq로 HEAD(실패 시 GET) 요청, 2xx/3xx만 PASS.
/// 네트워크 오류(타임아웃 등)는 FAIL이 아니라 WARN — "죽은 링크"와 "확인 불가"를 구분(design-spec.md 원칙과 동일 취지).
fn dead_link_check(input: &Input, skip: bool) -> CheckResult {
    if skip {
        return CheckResult {
            id: "dead_link".into(),
            title: "인용 URL 응답 확인".into(),
            status: CheckStatus::NotConfigured,
            evidence: "--skip-link-check 지정됨".into(),
        };
    }
    if input.citations.is_empty() {
        return CheckResult {
            id: "dead_link".into(),
            title: "인용 URL 응답 확인".into(),
            status: CheckStatus::NotApplicable,
            evidence: "인용 없음".into(),
        };
    }
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(8)).build();
    let mut dead: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for c in &input.citations {
        let ok = match agent.head(&c.url).call() {
            Ok(resp) => resp.status() < 400,
            Err(ureq::Error::Status(code, _)) => code < 400,
            Err(_) => {
                unknown.push(c.url.clone());
                continue;
            }
        };
        if !ok {
            dead.push(c.url.clone());
        }
    }
    if dead.is_empty() && unknown.is_empty() {
        CheckResult {
            id: "dead_link".into(),
            title: "인용 URL 응답 확인".into(),
            status: CheckStatus::Pass,
            evidence: format!("{}건 모두 응답 정상", input.citations.len()),
        }
    } else if !dead.is_empty() {
        CheckResult {
            id: "dead_link".into(),
            title: "인용 URL 응답 확인".into(),
            status: CheckStatus::Fail,
            evidence: format!("데드링크 {}건: {}", dead.len(), dead.join(", ")),
        }
    } else {
        CheckResult {
            id: "dead_link".into(),
            title: "인용 URL 응답 확인".into(),
            status: CheckStatus::Warn,
            evidence: format!("응답 확인 불가(타임아웃 등) {}건: {}", unknown.len(), unknown.join(", ")),
        }
    }
}

pub struct CheckOptions {
    pub as_of_year: u32,
    pub skip_link_check: bool,
}

pub fn run_all(spec: &Spec, input: &Input, opts: &CheckOptions) -> Vec<CheckResult> {
    let all = vec![
        citation_density_check(input),
        source_diversity_check(spec, input),
        numeric_consistency_check(input),
        access_limitation_disclosure_check(input),
        incentive_disclosure_scan(input),
        staleness_flag(spec, input, opts.as_of_year),
        dead_link_check(input, opts.skip_link_check),
    ];
    all.into_iter().filter(|r| spec.check_enabled(&r.id)).collect()
}

/// report.rs가 spec.deterministic_checks 목록과 대조해 표를 그릴 수 있도록 JSON으로 직렬화.
pub fn to_json(results: &[CheckResult]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for r in results {
        map.insert(r.id.clone(), serde_json::json!({"status": r.status.label(), "evidence": r.evidence}));
    }
    serde_json::Value::Object(map)
}
