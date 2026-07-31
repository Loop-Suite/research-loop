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

/// PASS/REVISE 2상태. P0 확정 finding, 결정론 FAIL, 커버리지 갭 중 하나라도 있으면 REVISE.
fn verdict(findings: &[Finding], resolved: &HashMap<String, Resolution>, checks: &[CheckResult], coverage_gap_count: usize) -> String {
    let confirmed: Vec<&Finding> = findings
        .iter()
        .filter(|f| resolved.get(&f.id).map(|r| r.status.as_str()) == Some("CONFIRMED"))
        .collect();

    if confirmed.iter().any(|f| f.severity == "P0" || f.severity == "P1") {
        return "REVISE".to_string();
    }
    if checks.iter().any(|c| c.status == CheckStatus::Fail) {
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
