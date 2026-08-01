use crate::checks::CheckResult;
use crate::describe::Describe;
use crate::discourse::{DiscourseAudit, Resolution};
use crate::fixcheck::FixStatus;
use crate::improve::Suggestion;
use crate::input::Input;
use crate::lens::{Finding, GoodThing};
use crate::quantify::QuantSummary;
use crate::requirements::AngleCheck;
use crate::spec::Spec;
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn severity_rank(s: &str) -> u8 {
    match s {
        "P0" => 0,
        "P1" => 1,
        "P2" => 2,
        "P3" => 3,
        _ => 4,
    }
}

fn checks_table(checks: &[CheckResult]) -> String {
    let mut md = String::new();
    md.push_str("| Check | Status | Evidence |\n|---|---|---|\n");
    for c in checks {
        md.push_str(&format!("| {} | {} | {} |\n", c.title, c.status.label(), c.evidence));
    }
    md
}

/// review 서브커맨드 결과를 렌더링하는 데 필요한 모든 입력.
pub struct ReportCtx<'a> {
    pub out_dir: &'a Path,
    pub spec: &'a Spec,
    pub input: &'a Input,
    pub selected_lenses: &'a [String],
    pub round: usize,
    pub findings: &'a [Finding],
    pub resolved: &'a HashMap<String, Resolution>,
    pub unverified: &'a [(String, String)],
    pub good_things: &'a [GoodThing],
    pub checks: &'a [CheckResult],
    pub angles: &'a Option<Vec<AngleCheck>>,
    pub coverage_gaps: &'a [String],
    pub audit: &'a [DiscourseAudit],
    pub quant: &'a QuantSummary,
    pub fix_results: &'a [FixStatus],
}

pub fn write(ctx: ReportCtx) -> Result<PathBuf> {
    let ReportCtx {
        out_dir, spec, input, selected_lenses, round, findings, resolved, unverified, good_things,
        checks, angles, coverage_gaps, audit, quant, fix_results,
    } = ctx;

    let mut md = String::new();

    md.push_str(&format!("# 리서치 문서 검증 — {} (round {})\n\n", spec.name, round));
    md.push_str(&format!(
        "**Verdict: {}**  ·  Score: {}/100  ·  섹션 {}개 · {}단어 · 인용 {}건\n\n",
        quant.verdict,
        quant.score,
        input.sections.len(),
        input.word_count,
        input.citations.len(),
    ));
    md.push_str(&format!("선택 렌즈: {}\n\n", selected_lenses.join(", ")));

    if !fix_results.is_empty() {
        md.push_str("## 이전 라운드 대비\n\n| Finding | Status | Evidence |\n|---|---|---|\n");
        for f in fix_results {
            md.push_str(&format!("| {} | {} | {} |\n", f.finding_id, f.status, f.evidence));
        }
        let reversed = fix_results.iter().filter(|f| f.status == "REVERSED").count();
        if reversed > 0 {
            md.push_str(&format!("\n⚠️ {}건이 REVERSED — 이전 라운드 결론이 최신 근거로 뒤집혔습니다(예: 티오더-KT 사례 유형). 해당 섹션을 우선 재검토하세요.\n", reversed));
        }
        md.push('\n');
    }

    md.push_str("## Deterministic checks\n\n");
    md.push_str(&checks_table(checks));
    md.push('\n');

    md.push_str("## 정량 요약\n\n");
    if quant.score_deductions.is_empty() {
        md.push_str("- 감점 없음 (CONFIRMED finding 없음)\n");
    } else {
        md.push_str("- 감점 근거:\n");
        for d in &quant.score_deductions {
            md.push_str(&format!("  - {}\n", d));
        }
    }
    md.push_str(&format!("- 커버리지 갭: {}건\n\n", quant.coverage_gap_count));

    md.push_str("## Coverage Verification (리서치 브리프 앵글 충족 여부)\n\n");
    md.push_str("REQ ID는 브리프를 코드가 결정론적으로 번호매김한 것 — LLM이 빠뜨린 REQ-ID는 코드가 강제로 MISSING 처리한다(#8).\n\n");
    match angles {
        None => md.push_str("(브리프 미제공 — 검증 생략)\n\n"),
        Some(list) if list.is_empty() => md.push_str("(앵글 없음)\n\n"),
        Some(list) => {
            md.push_str("| REQ ID | Angle | Status | Evidence or gap |\n|---|---|---|---|\n");
            for a in list {
                md.push_str(&format!("| {} | {} | {} | {} |\n", a.req_id, a.angle, a.status, a.evidence));
            }
            md.push('\n');
        }
    }
    if !coverage_gaps.is_empty() {
        md.push_str("### coverage_gaps\n\n");
        for g in coverage_gaps {
            md.push_str(&format!("- {}\n", g));
        }
        md.push('\n');
    }

    let mut confirmed: Vec<&Finding> = findings
        .iter()
        .filter(|f| resolved.get(&f.id).map(|r| r.status.as_str()) == Some("CONFIRMED"))
        .collect();
    confirmed.sort_by_key(|f| severity_rank(&f.severity));

    // citation_status 요약 — 이제 LLM 자기판정이 아니라 checks::verify_citations가 실제 HTTP
    // 재요청 + 인용 문구 대조로 코드가 직접 산정한 값이다(#4). CONFIRMED finding 기준.
    let mut citation_summary: HashMap<&str, usize> = HashMap::new();
    for f in &confirmed {
        *citation_summary.entry(f.citation_status.as_str()).or_insert(0) += 1;
    }
    md.push_str("## Citation Status 요약 (코드가 실제 원문 재요청·대조로 산정, LLM 판정 아님)\n\n");
    if citation_summary.is_empty() {
        md.push_str("(확정 finding 없음)\n\n");
    } else {
        md.push_str("| Status | Count |\n|---|---|\n");
        for k in ["UNFETCHED", "FETCH_FAILED", "QUOTE_MATCHED", "QUOTE_NOT_FOUND"] {
            let n = citation_summary.get(k).copied().unwrap_or(0);
            md.push_str(&format!("| {} | {} |\n", k, n));
        }
        md.push('\n');
    }

    md.push_str("## Findings\n\n");
    md.push_str(&format!("허용 label: {}\n\n", spec.labels_prompt()));
    md.push_str(
        "| ID | Priority | Label | Lens | Reviewer | Section | Citation | Citation Status (code-verified) | LLM Citation Status (advisory) | Claim | Evidence | Recommendation | Discourse result |\n\
         |---|---|---|---|---|---|---|---|---|---|---|---|---|\n",
    );
    for f in &confirmed {
        let r = resolved.get(&f.id);
        let discourse_result = match r {
            Some(res) if res.needs_human_review => format!("\u{26a0} HUMAN REVIEW REQUIRED — {}", res.reason),
            Some(res) => res.reason.clone(),
            None => String::new(),
        };
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            f.id, f.severity, f.label, f.lens, f.reviewer, f.section, f.citation_ref, f.citation_status,
            f.llm_citation_status, f.claim, f.evidence, f.recommendation, discourse_result
        ));
    }
    md.push('\n');

    let rejected: Vec<&Finding> = findings
        .iter()
        .filter(|f| resolved.get(&f.id).map(|r| r.status.as_str()) == Some("REJECTED"))
        .collect();
    if !rejected.is_empty() {
        md.push_str("### 기각된 후보\n\n");
        for f in &rejected {
            let reason = resolved.get(&f.id).map(|r| r.reason.as_str()).unwrap_or("");
            md.push_str(&format!("- {} ({}) — {}\n", f.id, f.section, reason));
        }
        md.push('\n');
    }

    if !unverified.is_empty() {
        md.push_str("### 검증 필요 사항 (근거 부족으로 finding 미승격)\n\n");
        for (lens_id, item) in unverified {
            md.push_str(&format!("- [{}] {}\n", lens_id, item));
        }
        md.push('\n');
    }

    md.push_str("## Good Things (유지할 리서치 관행)\n\n");
    if good_things.is_empty() {
        md.push_str("관찰되지 않음\n\n");
    } else {
        md.push_str("| Section | Good practice | Why it should be preserved |\n|---|---|---|\n");
        for g in good_things {
            md.push_str(&format!("| {} | {} | {} |\n", g.section, g.practice, g.why));
        }
        md.push('\n');
    }

    md.push_str("## Discourse audit\n\n");
    md.push_str("| Round | Move | Lens | Target | Detail | New evidence |\n|---|---|---|---|---|---|\n");
    for a in audit {
        for m in &a.moves {
            md.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                a.round, m.kind, m.lens, m.target, m.detail, m.new_evidence
            ));
        }
    }

    let path = out_dir.join("report.md");
    std::fs::write(&path, md).with_context(|| format!("{} 쓰기 실패", path.display()))?;
    Ok(path)
}

pub fn write_describe(out_dir: &Path, d: &Describe, todos: &[String]) -> Result<PathBuf> {
    let mut md = String::new();
    md.push_str(&format!("# {}\n\n{}\n\n", d.title, d.summary));
    md.push_str("## Key Findings\n\n");
    for w in &d.key_findings {
        md.push_str(&format!("- {}\n", w));
    }
    md.push_str(&format!("\n## Labels\n\n{}\n\n", d.labels.join(", ")));
    md.push_str(&format!("## can_be_split\n\n{} — {}\n\n", d.can_be_split, d.can_be_split_note));
    md.push_str("## 확인 필요 마커 (결정론적 스캔)\n\n");
    if todos.is_empty() {
        md.push_str("없음\n");
    } else {
        for t in todos {
            md.push_str(&format!("- {}\n", t));
        }
    }
    let path = out_dir.join("describe.md");
    std::fs::write(&path, md).with_context(|| format!("{} 쓰기 실패", path.display()))?;
    Ok(path)
}

pub fn write_improve(out_dir: &Path, suggestions: &[Suggestion]) -> Result<PathBuf> {
    let mut md = String::new();
    md.push_str("# 리서치 문서 개정 제안\n\n");
    if suggestions.is_empty() {
        md.push_str("제안 없음\n");
    }
    for s in suggestions {
        md.push_str(&format!("## {} — {} [{}]\n\n", s.relevant_section, s.one_sentence_summary, s.label));
        md.push_str(&format!("{}\n\n", s.suggestion_content));
        md.push_str(&format!("```markdown\n// before\n{}\n```\n\n", s.existing_text));
        md.push_str(&format!("```markdown\n// after\n{}\n```\n\n", s.revised_text));
    }
    let path = out_dir.join("improve.md");
    std::fs::write(&path, md).with_context(|| format!("{} 쓰기 실패", path.display()))?;
    Ok(path)
}
