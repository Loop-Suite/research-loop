//! 결정론적(LLM 미사용) 검사. codereview-loop의 policy.rs+semgrep.rs를 통합한 것 —
//! 리서치 도메인에는 "외부 결정론 도구가 채워주는 자동 스캐너"(semgrep 대응물)가 없어서
//! 굳이 두 모듈로 나눌 이유가 없다는 판단(docs/design-spec.md §3 "semgrep 대응 구조적 차이" 참조).
//! docs/research-and-evidence-survey §2에서 관찰한 6개 실패모드에 1:1 대응한다.

use crate::input::Input;
use crate::lens::Finding;
use crate::spec::Spec;
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, ToSocketAddrs};
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

    fn from_label(s: &str) -> Result<CheckStatus> {
        match s {
            "PASS" => Ok(CheckStatus::Pass),
            "WARN" => Ok(CheckStatus::Warn),
            "FAIL" => Ok(CheckStatus::Fail),
            "N/A" => Ok(CheckStatus::NotApplicable),
            "NOT_CONFIGURED" => Ok(CheckStatus::NotConfigured),
            other => Err(anyhow!("알 수 없는 check status: \"{other}\" (PASS|WARN|FAIL|N/A|NOT_CONFIGURED 중 하나여야 함)")),
        }
    }
}

pub struct CheckResult {
    pub id: String,
    pub title: String,
    pub status: CheckStatus,
    pub evidence: String,
}

/// 실패모드1: 일반. 근사 문장 수 대비 인용 수 비율.
/// 두 가지 휴리스틱(한국어 종결어미 목록 / 일반 문장부호 `.!?`)의 max를 취해 어느 한쪽이
/// 놓치는 경우(영문 문서, 어미 목록에 없는 종결형 등)를 서로 보완한다.
/// 가정: 형태소 분석이 아니라 휴리스틱이다 — 근사치로만 사용(불확실, #5).
fn approx_sentence_count(doc: &str) -> usize {
    let endings = [
        "다.", "음.", "됨.", "함.", "임.", "라.", "니다.", "습니다.", "입니다.", "네요.", "어요.", "예요.",
    ];
    let ending_hits: usize = endings.iter().map(|e| doc.matches(e).count()).sum();

    // 숫자 사이 소수점("3.5")이나 뒤에 공백이 없는 약어성 표기는 배제하기 위해
    // "문장부호 앞이 숫자가 아니고, 뒤에 공백/개행이 오는" 경우만 카운트.
    let punct_re = Regex::new(r"[^0-9][.!?](?:\s|$)").expect("sentence punctuation regex 컴파일 실패");
    let punct_hits = punct_re.find_iter(doc).count();

    ending_hits.max(punct_hits)
}

fn citation_density_check(input: &Input) -> CheckResult {
    let approx_sentences = approx_sentence_count(&input.document);
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

/// URL의 host가 `domain`(또는 그 서브도메인)과 정확히 일치하는지 판정.
/// `url` crate로 실제 host를 파싱한 뒤 비교하므로, 기존의 `url.contains(domain)` 방식이 갖던
/// 오탐(예: "evil-tossplace.com.attacker.net" 같은 문자열이 우연히 domain을 부분 포함하는 경우)을 없앤다(#5).
/// 완전한 public-suffix 기반 registrable domain 계산은 아니지만(psl 크레이트 미도입), host 정확 일치/서브도메인
/// 일치로 substring 오탐은 제거한다.
fn host_matches_owned_domain(url_str: &str, domain: &str) -> bool {
    let host = match url::Url::parse(url_str) {
        Ok(u) => match u.host_str() {
            Some(h) => h.trim_end_matches('.').to_ascii_lowercase(),
            None => return false,
        },
        Err(_) => return false,
    };
    let domain = domain.trim_end_matches('.').to_ascii_lowercase();
    host == domain || host.ends_with(&format!(".{domain}"))
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
        .filter(|c| spec.subject_owned_domains.iter().any(|d| host_matches_owned_domain(&c.url, d)))
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

// ---------------------------------------------------------------------------
// SSRF 방어(#11): dead_link_check·citation quote 검증 모두 이 아래 안전한 fetch 경로만 사용한다.
// ---------------------------------------------------------------------------

const MAX_REDIRECTS: u32 = 5;
const MAX_BODY_BYTES: usize = 1_000_000; // 1MB

/// IPv4 loopback(127.0.0.0/8) / private(10/8, 172.16/12, 192.168/16) /
/// link-local(169.254.0.0/16 — 클라우드 metadata 169.254.169.254 포함) /
/// multicast·reserved(224.0.0.0/4, 240.0.0.0/4) / unspecified(0.0.0.0) 차단.
/// `Ipv4Addr::is_private`류 표준 메서드 대신 옥텟을 직접 비교 — 크레이트/Rust 버전에 따른
/// 안정성 문제 없이 항상 동일하게 동작하도록 명시적으로 작성.
fn ipv4_is_blocked(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    o[0] == 127
        || o[0] == 10
        || (o[0] == 172 && (16..=31).contains(&o[1]))
        || (o[0] == 192 && o[1] == 168)
        || (o[0] == 169 && o[1] == 254)
        || o[0] >= 224
        || o == [0, 0, 0, 0]
}

/// IPv6 loopback(::1) / unspecified(::) / link-local(fe80::/10) / unique-local(fc00::/7) /
/// multicast(ff00::/8) 차단. IPv4-mapped(::ffff:a.b.c.d)는 내부 IPv4로 환산해 재검사.
fn ipv6_is_blocked(ip: Ipv6Addr) -> bool {
    if ip.is_loopback() || ip.is_unspecified() {
        return true;
    }
    if let Some(v4) = ip.to_ipv4() {
        return ipv4_is_blocked(v4);
    }
    let seg0 = ip.segments()[0];
    (seg0 & 0xff00) == 0xff00 // multicast ff00::/8
        || (seg0 & 0xffc0) == 0xfe80 // link-local fe80::/10
        || (seg0 & 0xfe00) == 0xfc00 // unique local fc00::/7
}

fn ip_is_blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => ipv4_is_blocked(v4),
        IpAddr::V6(v6) => ipv6_is_blocked(v6),
    }
}

/// host(도메인 또는 IP literal)를 실제로 DNS resolve해서 모든 응답 IP를 검사한다(DNS rebinding 방지 —
/// 호스트명만 보고 통과시키지 않고 매번 실제로 붙게 될 IP를 확인).
fn resolve_and_validate(host: &str, port: u16) -> Result<()> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        anyhow::ensure!(!ip_is_blocked(ip), "차단된 IP 대역: {ip}");
        return Ok(());
    }
    let addrs = (host, port)
        .to_socket_addrs()
        .with_context(|| format!("DNS 해석 실패: {host}"))?;
    let mut any = false;
    for addr in addrs {
        any = true;
        anyhow::ensure!(!ip_is_blocked(addr.ip()), "차단된 IP 대역으로 해석됨: {} -> {}", host, addr.ip());
    }
    anyhow::ensure!(any, "DNS 해석 결과 없음: {host}");
    Ok(())
}

/// URL 파싱 + 스킴(http/https만 허용) + host resolve/차단 대역 검증. 최초 요청뿐 아니라
/// redirect 각 hop마다 다시 호출해 SSRF 우회(리다이렉트로 내부망 유도)를 막는다.
fn validate_url_safe(raw_url: &str) -> Result<url::Url> {
    let u = url::Url::parse(raw_url).with_context(|| format!("URL 파싱 실패: {raw_url}"))?;
    anyhow::ensure!(matches!(u.scheme(), "http" | "https"), "허용되지 않은 스킴: {}", u.scheme());
    let host = u.host_str().ok_or_else(|| anyhow!("URL에 host 없음: {raw_url}"))?;
    let port = u.port_or_known_default().unwrap_or(if u.scheme() == "https" { 443 } else { 80 });
    resolve_and_validate(host, port)?;
    Ok(u)
}

struct FetchOutcome {
    status: u16,
    content_type: Option<String>,
    body: Option<Vec<u8>>, // GET일 때만 Some
}

fn read_bounded(resp: ureq::Response, max_bytes: usize) -> Result<Vec<u8>> {
    let mut reader = resp.into_reader().take(max_bytes as u64 + 1);
    let mut buf = Vec::new();
    reader.read_to_end(&mut buf).context("응답 본문 읽기 실패")?;
    buf.truncate(max_bytes);
    Ok(buf)
}

/// SSRF 방어가 적용된 HEAD/GET 요청. redirect는 ureq의 자동 추적을 끄고(`.redirects(0)`) 직접
/// 루프를 돌며 매 hop마다 [`validate_url_safe`]를 다시 통과시킨다. GET은 응답 본문을 1MB로 제한.
fn safe_fetch(raw_url: &str, method_get: bool) -> Result<FetchOutcome> {
    let mut current = validate_url_safe(raw_url)?;
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(8)).redirects(0).build();
    for hop in 0..=MAX_REDIRECTS {
        let req = if method_get { agent.get(current.as_str()) } else { agent.head(current.as_str()) };
        let resp = match req.call() {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(anyhow!("요청 실패: {e}")),
        };
        let status = resp.status();
        if (300..400).contains(&status) {
            anyhow::ensure!(hop < MAX_REDIRECTS, "리다이렉트 한도({}) 초과", MAX_REDIRECTS);
            let location = resp
                .header("Location")
                .ok_or_else(|| anyhow!("리다이렉트 응답({status})에 Location 헤더 없음"))?
                .to_string();
            let next = current.join(&location).with_context(|| format!("리다이렉트 URL 해석 실패: {location}"))?;
            current = validate_url_safe(next.as_str())?; // 매 hop 재검증 — SSRF 우회 방지
            continue;
        }
        let content_type = resp.header("Content-Type").map(|s| s.to_string());
        let body = if method_get { Some(read_bounded(resp, MAX_BODY_BYTES)?) } else { None };
        return Ok(FetchOutcome { status, content_type, body });
    }
    Err(anyhow!("리다이렉트 처리 실패"))
}

enum Probe {
    Status(u16),
    Err(String),
}

fn probe(url: &str, get: bool) -> Probe {
    match safe_fetch(url, get) {
        Ok(o) => Probe::Status(o.status),
        Err(e) => Probe::Err(e.to_string()),
    }
}

enum LinkStatus {
    Ok,
    /// HEAD·GET 둘 다 실제 HTTP 응답을 받았고 그 상태코드가 확정적으로 4xx/5xx인 경우만.
    Dead(String),
    /// 전송 오류(타임아웃/DNS 실패/SSRF 차단 등) — "죽었다"고 단정할 수 없는 경우.
    Unreachable(String),
}

/// HEAD로 먼저 확인하고, 실패하거나(전송 오류) 에러 상태를 반환하면 GET으로 재시도한다
/// (README가 명시하는 "HEAD 실패 시 GET fallback" 계약 — 이전 구현엔 실제로 없었음, #11).
/// HEAD를 지원하지 않는 사이트(405 등)가 GET 재시도로 정상 판정되도록 하는 것이 목적.
fn check_one(url: &str) -> LinkStatus {
    match probe(url, false) {
        Probe::Status(s) if s < 400 => LinkStatus::Ok,
        Probe::Status(head_status) => match probe(url, true) {
            Probe::Status(s2) if s2 < 400 => LinkStatus::Ok,
            Probe::Status(s2) => LinkStatus::Dead(format!("HEAD={head_status}, GET={s2}")),
            Probe::Err(e) => LinkStatus::Unreachable(format!("HEAD={head_status}, GET 오류: {e}")),
        },
        Probe::Err(head_err) => match probe(url, true) {
            Probe::Status(s2) if s2 < 400 => LinkStatus::Ok,
            Probe::Status(s2) => LinkStatus::Unreachable(format!("HEAD 오류({head_err}), GET={s2}")),
            Probe::Err(get_err) => LinkStatus::Unreachable(format!("HEAD/GET 모두 오류: {head_err} / {get_err}")),
        },
    }
}

/// 실측 데드링크 확인. SSRF 방어된 HEAD(실패 시 GET fallback) 요청, 2xx/3xx만 PASS.
/// 네트워크 오류·SSRF 차단·타임아웃은 FAIL이 아니라 WARN — "죽은 링크"와 "확인 불가"를 구분(design-spec.md 원칙과 동일 취지).
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
    let mut dead: Vec<String> = Vec::new();
    let mut unknown: Vec<String> = Vec::new();
    for c in &input.citations {
        match check_one(&c.url) {
            LinkStatus::Ok => {}
            LinkStatus::Dead(detail) => dead.push(format!("{} ({detail})", c.url)),
            LinkStatus::Unreachable(detail) => unknown.push(format!("{} ({detail})", c.url)),
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
            evidence: format!("응답 확인 불가(타임아웃/차단 등) {}건: {}", unknown.len(), unknown.join(", ")),
        }
    }
}

// ---------------------------------------------------------------------------
// citation_status 실측 검증(#4) — LLM 자기판정을 코드가 덮어쓴다.
// ---------------------------------------------------------------------------

pub enum CitationVerification {
    /// --skip-link-check 지정됨, citation_ref가 UNKNOWN/파싱 불가, 또는 대조할 인용문(evidence)이 없음.
    Unfetched,
    /// 요청 실패, SSRF 차단, 비-2xx 응답, 또는 텍스트가 아닌 콘텐츠 타입(PDF/이미지 등)이라 대조 불가.
    FetchFailed,
    /// 본문(정규화 후)에 finding.evidence(인용 문구로 취급)가 포함됨을 확인.
    QuoteMatched,
    /// 원문은 정상 fetch됐지만 인용 문구가 본문에서 발견되지 않음.
    QuoteNotFound,
}

impl CitationVerification {
    pub fn label(&self) -> &'static str {
        match self {
            CitationVerification::Unfetched => "UNFETCHED",
            CitationVerification::FetchFailed => "FETCH_FAILED",
            CitationVerification::QuoteMatched => "QUOTE_MATCHED",
            CitationVerification::QuoteNotFound => "QUOTE_NOT_FOUND",
        }
    }
}

fn is_text_content_type(ct: &str) -> bool {
    let ct = ct.to_ascii_lowercase();
    ct.contains("text") || ct.contains("json") || ct.contains("xml") || ct.contains("html")
}

/// 공백 제거 + 소문자화만 하는 얕은 정규화. 형태소/구두점 차이까지 흡수하지는 못하지만
/// (완전한 텍스트 정규화는 스코프 밖), 개행·띄어쓰기 차이로 인한 오탐은 줄인다.
fn normalize_for_match(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).flat_map(|c| c.to_lowercase()).collect()
}

fn verify_citation(input: &Input, citation_ref: &str, quote: &str, skip: bool) -> CitationVerification {
    if skip {
        return CitationVerification::Unfetched;
    }
    let idx: usize = match citation_ref.trim().parse() {
        Ok(n) => n,
        Err(_) => return CitationVerification::Unfetched,
    };
    let citation = match input.citations.iter().find(|c| c.index == idx) {
        Some(c) => c,
        None => return CitationVerification::Unfetched,
    };
    if quote.trim().is_empty() {
        return CitationVerification::Unfetched;
    }
    match safe_fetch(&citation.url, true) {
        Ok(outcome) if outcome.status < 400 => {
            let is_text = outcome.content_type.as_deref().map(is_text_content_type).unwrap_or(true);
            if !is_text {
                return CitationVerification::FetchFailed;
            }
            let body = match outcome.body {
                Some(b) => b,
                None => return CitationVerification::FetchFailed,
            };
            let body_text = String::from_utf8_lossy(&body);
            if normalize_for_match(&body_text).contains(&normalize_for_match(quote)) {
                CitationVerification::QuoteMatched
            } else {
                CitationVerification::QuoteNotFound
            }
        }
        _ => CitationVerification::FetchFailed,
    }
}

/// findings의 citation_status를 코드가 직접 재산정해 덮어쓴다. LLM이 원래 채운 값은
/// `llm_citation_status`에 참고용으로만 보존(#4) — evidence 필드를 "인용 문구"로 취급해
/// 실제 원문과 대조한다(finding 스키마에 별도 quote 필드가 없어 evidence를 대용으로 사용).
pub fn verify_citations(input: &Input, findings: &mut [Finding], skip: bool) {
    for f in findings.iter_mut() {
        let verified = verify_citation(input, &f.citation_ref, &f.evidence, skip);
        f.llm_citation_status = f.citation_status.clone();
        f.citation_status = verified.label().to_string();
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
/// `--deterministic-results`로 다시 읽어들일 때 [`from_json`]이 역직렬화할 수 있는 형식이다.
pub fn to_json(results: &[CheckResult]) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for r in results {
        map.insert(r.id.clone(), serde_json::json!({"title": r.title, "status": r.status.label(), "evidence": r.evidence}));
    }
    serde_json::Value::Object(map)
}

/// `--deterministic-results` 외부 JSON을 역직렬화(#6). 최소 스키마 검증: object여야 하고,
/// 비어있지 않아야 하며, 각 항목은 알려진 status 라벨을 가진 "status" 필드를 반드시 가져야 한다.
/// title이 없으면(외부 도구가 직접 손으로 작성한 파일 등) id를 title로 대체한다.
pub fn from_json(v: &serde_json::Value) -> Result<Vec<CheckResult>> {
    let obj = v.as_object().ok_or_else(|| anyhow!("deterministic_results는 JSON object여야 함"))?;
    anyhow::ensure!(!obj.is_empty(), "deterministic_results가 비어있음");
    let mut out = Vec::new();
    for (id, entry) in obj {
        let status_str = entry
            .get("status")
            .and_then(|s| s.as_str())
            .ok_or_else(|| anyhow!("check \"{id}\"에 status 필드 없음(또는 문자열 아님)"))?;
        let status = CheckStatus::from_label(status_str).with_context(|| format!("check \"{id}\""))?;
        let evidence = entry.get("evidence").and_then(|e| e.as_str()).unwrap_or("").to_string();
        let title = entry
            .get("title")
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| id.clone());
        out.push(CheckResult { id: id.clone(), title, status, evidence });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_loopback_private_linklocal_and_metadata() {
        assert!(ipv4_is_blocked(Ipv4Addr::new(127, 0, 0, 1)));
        assert!(ipv4_is_blocked(Ipv4Addr::new(10, 1, 2, 3)));
        assert!(ipv4_is_blocked(Ipv4Addr::new(172, 16, 0, 1)));
        assert!(ipv4_is_blocked(Ipv4Addr::new(172, 31, 255, 255)));
        assert!(!ipv4_is_blocked(Ipv4Addr::new(172, 32, 0, 1)));
        assert!(ipv4_is_blocked(Ipv4Addr::new(192, 168, 1, 1)));
        assert!(ipv4_is_blocked(Ipv4Addr::new(169, 254, 169, 254))); // 클라우드 metadata
        assert!(ipv4_is_blocked(Ipv4Addr::new(0, 0, 0, 0)));
        assert!(!ipv4_is_blocked(Ipv4Addr::new(8, 8, 8, 8)));
        assert!(!ipv4_is_blocked(Ipv4Addr::new(93, 184, 216, 34)));
    }

    #[test]
    fn blocks_ipv6_loopback_linklocal_uniquelocal() {
        assert!(ipv6_is_blocked("::1".parse().unwrap()));
        assert!(ipv6_is_blocked("fe80::1".parse().unwrap()));
        assert!(ipv6_is_blocked("fc00::1".parse().unwrap()));
        assert!(!ipv6_is_blocked("2606:4700:4700::1111".parse().unwrap()));
    }

    #[test]
    fn rejects_disallowed_schemes() {
        assert!(validate_url_safe("file:///etc/passwd").is_err());
        assert!(validate_url_safe("ftp://example.com/a").is_err());
    }

    #[test]
    fn rejects_literal_private_ip_url() {
        assert!(validate_url_safe("http://127.0.0.1/admin").is_err());
        assert!(validate_url_safe("http://169.254.169.254/latest/meta-data/").is_err());
        assert!(validate_url_safe("http://192.168.0.1/").is_err());
    }

    #[test]
    fn from_json_roundtrips_to_json() {
        let results = vec![CheckResult {
            id: "dead_link".into(),
            title: "인용 URL 응답 확인".into(),
            status: CheckStatus::Warn,
            evidence: "test".into(),
        }];
        let v = to_json(&results);
        let back = from_json(&v).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].id, "dead_link");
        assert_eq!(back[0].status, CheckStatus::Warn);
        assert_eq!(back[0].title, "인용 URL 응답 확인");
    }

    #[test]
    fn from_json_rejects_unknown_status() {
        let v = serde_json::json!({"x": {"status": "MAYBE", "evidence": "e"}});
        assert!(from_json(&v).is_err());
    }

    #[test]
    fn from_json_rejects_non_object() {
        let v = serde_json::json!([1, 2, 3]);
        assert!(from_json(&v).is_err());
    }

    #[test]
    fn host_matches_owned_domain_rejects_substring_lookalike() {
        assert!(host_matches_owned_domain("https://tossplace.com/x", "tossplace.com"));
        assert!(host_matches_owned_domain("https://www.tossplace.com/x", "tossplace.com"));
        assert!(!host_matches_owned_domain("https://evil-tossplace.com.attacker.net/x", "tossplace.com"));
        assert!(!host_matches_owned_domain("https://nottossplace.com/x", "tossplace.com"));
    }

    #[test]
    fn approx_sentence_count_ignores_decimal_points() {
        // "3.5" 같은 소수점은 문장 종결로 세지 않아야 함.
        let doc = "매출은 3.5억 원이다. 성장률은 12.3% 였다.";
        // "이다." 는 종결어미 목록에 없으므로 punct 방식이 잡아야 함(최소 1 이상).
        assert!(approx_sentence_count(doc) >= 1);
    }
}
