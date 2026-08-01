use crate::checks::{CheckResult, CheckStatus};
use crate::discourse::Resolution;
use crate::input::Input;
use crate::lens::Finding;
use std::collections::HashMap;

pub struct QuantSummary {
    pub verdict: String, // PASS|REVISE — docs/design-spec.md §6 (codereview의 4상태 verdict보다 단순화)
    pub score: i64,       // 0-100
    pub score_deductions: Vec<String>,
    pub coverage_gap_count: usize,
}

fn severity_penalty(severity: &str) -> i64 {
    match severity {
        "P0" => 25,
        "P1" => 12,
        "P2" => 5,
        "P3" => 1,
        _ => 0,
    }
}

/// 확인된(CONFIRMED) finding만으로 100점에서 감점.
/// 가정: 감점폭은 codereview-loop과 동일 숫자를 유지(확장 금지, docs/design-spec.md §6).
fn score(findings: &[Finding], resolved: &HashMap<String, Resolution>) -> (i64, Vec<String>) {
    let mut total = 100i64;
    let mut deductions = Vec::new();
    for f in findings {
        if resolved.get(&f.id).map(|r| r.status.as_str()) == Some("CONFIRMED") {
            let p = severity_penalty(&f.severity);
            total -= p;
            deductions.push(format!("[{}] {} -{}점 — {}", f.severity, f.section, p, f.claim));
        }
    }
    (total.max(0), deductions)
}

/// PASS/REVISE 2상태.
///
/// #3: 결정론 체크(checks.rs)의 FAIL은 "hard evidence"다 — LLM이 자기신고한 confidence(discourse.rs
/// confidence_weight)가 아무리 높은 AGREE를 쌓아 어떤 finding을 REJECTED로 밀어내더라도, 그 finding과
/// 무관하게 checks 자체가 FAIL이면 이 함수는 무조건 REVISE를 반환한다 — findings/resolved 상태를 전혀
/// 참조하지 않는 독립 조건이라 confidence 가중치로 우회할 방법이 없다(quantify_tests::
/// hard_evidence_check_fail_forces_revise_regardless_of_findings로 고정).
///
/// #7: needs_human_review가 선 resolution(--prior 재검사에서 UNKNOWN/REVERSED로 나온 finding)이
/// 하나라도 있으면, 그 finding의 severity와 무관하게 REVISE — "확인 불가"를 자동으로 통과시키지 않는다.
fn verdict(findings: &[Finding], resolved: &HashMap<String, Resolution>, checks: &[CheckResult], coverage_gap_count: usize) -> String {
    // 1순위: 결정론 체크 FAIL — findings/confidence와 무관하게 항상 우선한다.
    if checks.iter().any(|c| c.status == CheckStatus::Fail) {
        return "REVISE".to_string();
    }
    // 2순위: 사람 확인이 필요하다고 명시적으로 플래그된 판정(#7 UNKNOWN/REVERSED).
    if resolved.values().any(|r| r.needs_human_review) {
        return "REVISE".to_string();
    }

    let confirmed: Vec<&Finding> = findings
        .iter()
        .filter(|f| resolved.get(&f.id).map(|r| r.status.as_str()) == Some("CONFIRMED"))
        .collect();

    if confirmed.iter().any(|f| f.severity == "P0" || f.severity == "P1") {
        return "REVISE".to_string();
    }
    if coverage_gap_count > 0 {
        return "REVISE".to_string();
    }
    "PASS".to_string()
}

pub fn summarize(
    _input: &Input,
    findings: &[Finding],
    resolved: &HashMap<String, Resolution>,
    checks: &[CheckResult],
    coverage_gap_count: usize,
) -> QuantSummary {
    let (sc, deductions) = score(findings, resolved);
    let v = verdict(findings, resolved, checks, coverage_gap_count);
    QuantSummary { verdict: v, score: sc, score_deductions: deductions, coverage_gap_count }
}

#[cfg(test)]
mod quantify_tests {
    use super::*;

    #[test]
    fn hard_evidence_check_fail_forces_revise_regardless_of_findings() {
        let findings: Vec<Finding> = Vec::new();
        let resolved: HashMap<String, Resolution> = HashMap::new();
        let checks = vec![CheckResult {
            id: "dead_link".into(),
            title: "인용 URL 응답 확인".into(),
            status: CheckStatus::Fail,
            evidence: "test".into(),
        }];
        let v = verdict(&findings, &resolved, &checks, 0);
        assert_eq!(
            v, "REVISE",
            "결정론 체크 FAIL은 confirmed finding이 하나도 없어도(=confidence 가중치의 영향 없이) 항상 REVISE를 강제해야 한다(#3)"
        );
    }

    #[test]
    fn needs_human_review_forces_revise_even_for_low_severity() {
        let findings: Vec<Finding> = Vec::new();
        let mut resolved: HashMap<String, Resolution> = HashMap::new();
        resolved.insert(
            "f1".to_string(),
            Resolution {
                finding_id: "f1".to_string(),
                status: "CONFIRMED".to_string(),
                merged_into: String::new(),
                reason: "unknown".to_string(),
                needs_human_review: true,
            },
        );
        let checks: Vec<CheckResult> = Vec::new();
        let v = verdict(&findings, &resolved, &checks, 0);
        assert_eq!(v, "REVISE", "needs_human_review 플래그가 선 resolution이 있으면 항상 REVISE여야 한다(#7)");
    }

    #[test]
    fn clean_run_is_pass() {
        let findings: Vec<Finding> = Vec::new();
        let resolved: HashMap<String, Resolution> = HashMap::new();
        let checks: Vec<CheckResult> = Vec::new();
        assert_eq!(verdict(&findings, &resolved, &checks, 0), "PASS");
    }
}
