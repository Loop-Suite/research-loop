mod ask;
mod checks;
mod describe;
mod discourse;
mod fixcheck;
mod improve;
mod input;
mod lens;
mod llm;
mod promptctx;
mod quantify;
mod report;
mod requirements;
mod spec;
mod state;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use lens::Finding;
use llm::Llm;
use spec::Spec;
use std::path::PathBuf;

#[derive(clap::ValueEnum, Clone, Debug, PartialEq)]
enum Backend {
    /// claude -p 서브프로세스
    Claude,
    /// OpenRouter REST API (OPENROUTER_API_KEY 필요)
    Openrouter,
}

#[derive(Parser, Debug)]
#[command(
    name = "research",
    version,
    about = "다각도(멀티 페르소나) 시장/경쟁사 리서치 문서 검증 — Code-Review-Loop을 리서치 도메인에 이식"
)]
struct Cli {
    #[arg(long, default_value = "claude", global = true)]
    claude_bin: String,
    #[arg(long, value_enum, default_value = "claude", global = true)]
    backend: Backend,
    #[arg(long, global = true)]
    model: Option<String>,
    /// 렌즈 선정·good things·커버리지 검증·fix check 등 단순 판정 단계에 쓸 저비용 모델.
    #[arg(long, global = true)]
    cheap_model: Option<String>,
    #[arg(long, default_value_t = 2, global = true)]
    retries: u32,
    #[arg(long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// 렌즈별 독립 리뷰 + discourse 교차검증(기본 파이프라인)
    Review {
        #[arg(long)]
        spec: PathBuf,
        /// 검증할 리서치 문서(마크다운)
        #[arg(long)]
        document: PathBuf,
        /// 리서치 브리프(반드시 다뤄야 할 앵글 목록)
        #[arg(long)]
        brief: Option<PathBuf>,
        /// 톤/포맷 가이드
        #[arg(long)]
        style: Option<PathBuf>,
        #[arg(long)]
        deterministic_results: Option<PathBuf>,
        /// 렌즈 수동 지정(콤마 구분). 미지정 시 LLM이 문서 성격 보고 선정.
        #[arg(long)]
        lenses: Option<String>,
        #[arg(long, default_value = "runs")]
        out: PathBuf,
        #[arg(long, default_value_t = 1)]
        concurrency: usize,
        /// discourse 최대 라운드 수
        #[arg(long, default_value_t = 2)]
        max_rounds: usize,
        /// 이전 라운드 --out 디렉터리(state.json). 지정 시 이전 확정 finding의 FIXED/STILL_OPEN/REVERSED 판정 추가.
        #[arg(long)]
        prior: Option<PathBuf>,
        /// staleness_flag 계산 기준 연도(YYYY). 미지정 시 4자리 정수 최댓값을 문서에서 추출해 근사.
        #[arg(long)]
        as_of_year: Option<u32>,
        /// dead_link_check(실제 HTTP 요청) 생략 — 네트워크 없는 환경/CI에서 사용.
        #[arg(long)]
        skip_link_check: bool,
    },
    /// 문서 요약·핵심발견·라벨·분리 가능 여부 + 확인필요 마커 스캔
    Describe {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        document: PathBuf,
        #[arg(long)]
        brief: Option<PathBuf>,
        #[arg(long)]
        style: Option<PathBuf>,
        #[arg(long, default_value = "runs")]
        out: PathBuf,
    },
    /// 구체적 개정 제안(추가조사 반영/정정)
    Improve {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        document: PathBuf,
        #[arg(long)]
        brief: Option<PathBuf>,
        #[arg(long)]
        style: Option<PathBuf>,
        #[arg(long, default_value = "runs")]
        out: PathBuf,
    },
    /// 문서에 대한 자유 질의(ask.md에 누적)
    Ask {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        document: PathBuf,
        #[arg(long)]
        brief: Option<PathBuf>,
        #[arg(long)]
        style: Option<PathBuf>,
        #[arg(long, default_value = "runs")]
        out: PathBuf,
        question: String,
    },
}

fn main() {
    if let Err(e) = real_main() {
        eprintln!("에러: {e:#}");
        std::process::exit(1);
    }
}

fn build_llm(cli: &Cli) -> Result<(Llm, Llm)> {
    let usage = Llm::new_usage_tracker();
    let cheap_model = cli.cheap_model.clone().or_else(|| cli.model.clone());
    let (main_llm, cheap_llm) = match cli.backend {
        Backend::Claude => (
            Llm::claude_cli(cli.claude_bin.clone(), cli.model.clone(), cli.retries, cli.verbose, usage.clone()),
            Llm::claude_cli(cli.claude_bin.clone(), cheap_model, cli.retries, cli.verbose, usage.clone()),
        ),
        Backend::Openrouter => (
            Llm::openrouter(cli.model.clone(), cli.retries, cli.verbose, usage.clone())?,
            Llm::openrouter(cheap_model, cli.retries, cli.verbose, usage.clone())?,
        ),
    };
    Ok((main_llm, cheap_llm))
}

fn real_main() -> Result<()> {
    let cli = Cli::parse();
    let (llm, cheap_llm) = build_llm(&cli)?;

    match &cli.cmd {
        Cmd::Review { spec, document, brief, style, deterministic_results, lenses, out, concurrency, max_rounds, prior, as_of_year, skip_link_check } => {
            run_review(&llm, &cheap_llm, spec, document, brief, style, deterministic_results, lenses, out, *concurrency, *max_rounds, prior, *as_of_year, *skip_link_check)
        }
        Cmd::Describe { spec, document, brief, style, out } => run_describe(&llm, spec, document, brief, style, out),
        Cmd::Improve { spec, document, brief, style, out } => run_improve(&llm, spec, document, brief, style, out),
        Cmd::Ask { spec, document, brief, style, out, question } => run_ask(&llm, spec, document, brief, style, out, question),
    }
}

fn default_as_of_year(document: &str) -> u32 {
    let re = regex::Regex::new(r"(19|20)\d{2}").expect("year regex 컴파일 실패");
    re.find_iter(document)
        .filter_map(|m| m.as_str().parse::<u32>().ok())
        .max()
        .unwrap_or(2026)
}

#[allow(clippy::too_many_arguments)]
fn run_review(
    llm: &Llm,
    cheap_llm: &Llm,
    spec_path: &PathBuf,
    document_path: &PathBuf,
    brief_path: &Option<PathBuf>,
    style_path: &Option<PathBuf>,
    deterministic_results_path: &Option<PathBuf>,
    lenses_arg: &Option<String>,
    out: &PathBuf,
    concurrency: usize,
    max_rounds: usize,
    prior: &Option<PathBuf>,
    as_of_year: Option<u32>,
    skip_link_check: bool,
) -> Result<()> {
    let sp = Spec::load(spec_path)?;
    let mut inp = input::normalize(document_path, brief_path, style_path, deterministic_results_path)?;
    const DOC_WARN_CHARS: usize = 300_000;
    if inp.document.len() > DOC_WARN_CHARS {
        eprintln!(
            "경고: 문서가 {}자로 큼 — 렌즈별 리뷰·discourse·커버리지 호출마다 전체가 재전송되어 토큰 비용이 커짐",
            inp.document.len()
        );
    }

    let as_of = as_of_year.unwrap_or_else(|| default_as_of_year(&inp.document));
    if inp.deterministic_results.is_none() {
        let results = checks::run_all(&sp, &inp, &checks::CheckOptions { as_of_year: as_of, skip_link_check });
        inp.deterministic_results = Some(checks::to_json(&results));
    }

    let out_dir = prepare_out(out)?;

    let prior_state = match prior {
        None => None,
        Some(p) => Some(state::load(p)?),
    };
    let round = prior_state.as_ref().map(|s| s.round + 1).unwrap_or(1);

    println!("리서치 검증 시작(round {}) — {} ({}개 섹션, {}단어, 인용 {}건)", round, sp.name, inp.sections.len(), inp.word_count, inp.citations.len());

    let optional_selected: Vec<String> = match lenses_arg {
        Some(s) => {
            let ids: Vec<String> = s.split(',').map(|x| x.trim().to_string()).filter(|x| !x.is_empty()).collect();
            for id in &ids {
                anyhow::ensure!(sp.lens_by_id(id).is_some(), "spec에 없는 렌즈 id: {id}");
            }
            ids
        }
        None => lens::select_lenses(cheap_llm, &sp, &inp)?,
    };
    let mut selected_ids: Vec<String> = optional_selected;
    for l in sp.always_lenses() {
        if l.id != "good_things" && !selected_ids.contains(&l.id) {
            selected_ids.push(l.id.clone());
        }
    }
    println!("선정 렌즈: {}", selected_ids.join(", "));

    let lens_outputs: Vec<(String, lens::LensOutput)> = par_map(concurrency, selected_ids.clone(), |id| {
        let out = lens::review_lens(llm, &sp, &inp, &id)?;
        println!("  렌즈 완료: {} — finding {}건, 미검증 {}건", id, out.findings.len(), out.unverified.len());
        Ok((id, out))
    })?;

    let mut findings: Vec<Finding> = Vec::new();
    let mut unverified: Vec<(String, String)> = Vec::new();
    for (id, out) in lens_outputs {
        findings.extend(out.findings);
        for u in out.unverified {
            unverified.push((id.clone(), u));
        }
    }

    let good_things = if sp.lens_by_id("good_things").is_some() {
        lens::review_good_things(cheap_llm, &sp, &inp)?.good_things
    } else {
        Vec::new()
    };

    let (audit, mut resolved) = if findings.is_empty() {
        println!("finding 없음 — discourse 생략");
        (Vec::new(), std::collections::HashMap::new())
    } else {
        println!("discourse 시작 (최대 {}라운드)", max_rounds);
        discourse::run(llm, &sp, &mut findings, max_rounds)?
    };

    let mut fix_results: Vec<fixcheck::FixStatus> = Vec::new();
    if let Some(ps) = &prior_state {
        let prior_confirmed: Vec<Finding> = ps
            .findings
            .iter()
            .filter(|f| ps.resolved.get(&f.id).map(|r| r.status == "CONFIRMED").unwrap_or(false))
            .cloned()
            .collect();
        fix_results = fixcheck::run(cheap_llm, &sp, &inp, &prior_confirmed)?;
        for fr in &fix_results {
            if fr.status == "STILL_OPEN" || fr.status == "REVERSED" {
                if let Some(orig) = prior_confirmed.iter().find(|f| f.id == fr.finding_id) {
                    findings.push(orig.clone());
                    resolved.insert(
                        orig.id.clone(),
                        discourse::Resolution {
                            finding_id: orig.id.clone(),
                            status: "CONFIRMED".to_string(),
                            merged_into: String::new(),
                            reason: format!("이전 라운드 대비 {}: {}", fr.status, fr.evidence),
                        },
                    );
                }
            }
        }
    }

    let checks_results = checks::run_all(&sp, &inp, &checks::CheckOptions { as_of_year: as_of, skip_link_check });

    let confirmed_refs: Vec<&Finding> = findings
        .iter()
        .filter(|f| resolved.get(&f.id).map(|r| r.status.as_str()) == Some("CONFIRMED"))
        .collect();
    let angles = requirements::verify(cheap_llm, &sp, &inp, &confirmed_refs)?;
    let coverage_gaps = requirements::coverage_gaps(&angles);

    let quant = quantify::summarize(&inp, &findings, &resolved, &checks_results, coverage_gaps.len());

    let path = report::write(report::ReportCtx {
        out_dir: &out_dir,
        spec: &sp,
        input: &inp,
        selected_lenses: &selected_ids,
        round,
        findings: &findings,
        resolved: &resolved,
        unverified: &unverified,
        good_things: &good_things,
        checks: &checks_results,
        angles: &angles,
        coverage_gaps: &coverage_gaps,
        audit: &audit,
        quant: &quant,
        fix_results: &fix_results,
    })?;

    state::write(&out_dir, &state::State { round, findings: findings.clone(), resolved: resolved.clone() })?;

    println!("\n종료 — verdict={} score={}/100 coverage_gaps={}", quant.verdict, quant.score, quant.coverage_gap_count);
    println!("리포트: {}", path.display());
    println!("다음 라운드: --prior {}", out_dir.display());
    println!("{}", llm.usage().summary());
    Ok(())
}

fn run_describe(llm: &Llm, spec_path: &PathBuf, document_path: &PathBuf, brief_path: &Option<PathBuf>, style_path: &Option<PathBuf>, out: &PathBuf) -> Result<()> {
    let sp = Spec::load(spec_path)?;
    let inp = input::normalize(document_path, brief_path, style_path, &None)?;
    let out_dir = prepare_out(out)?;
    let d = describe::run(llm, &sp, &inp)?;
    let todos = describe::todo_sections(&inp.document);
    let path = report::write_describe(&out_dir, &d, &todos)?;
    println!("describe 완료: {}", path.display());
    println!("{}", llm.usage().summary());
    Ok(())
}

fn run_improve(llm: &Llm, spec_path: &PathBuf, document_path: &PathBuf, brief_path: &Option<PathBuf>, style_path: &Option<PathBuf>, out: &PathBuf) -> Result<()> {
    let sp = Spec::load(spec_path)?;
    let inp = input::normalize(document_path, brief_path, style_path, &None)?;
    let out_dir = prepare_out(out)?;
    let suggestions = improve::run(llm, &sp, &inp)?;
    let path = report::write_improve(&out_dir, &suggestions)?;
    println!("improve 완료: 제안 {}건 — {}", suggestions.len(), path.display());
    println!("{}", llm.usage().summary());
    Ok(())
}

fn run_ask(llm: &Llm, spec_path: &PathBuf, document_path: &PathBuf, brief_path: &Option<PathBuf>, style_path: &Option<PathBuf>, out: &PathBuf, question: &str) -> Result<()> {
    let sp = Spec::load(spec_path)?;
    let inp = input::normalize(document_path, brief_path, style_path, &None)?;
    let out_dir = prepare_out(out)?;
    let answer = ask::run(llm, &sp, &inp, question)?;
    let path = out_dir.join("ask.md");
    let mut existing = std::fs::read_to_string(&path).unwrap_or_default();
    existing.push_str(&format!("\n## Q: {question}\n\n{answer}\n"));
    std::fs::write(&path, existing).with_context(|| format!("{} 쓰기 실패", path.display()))?;
    println!("{}", answer);
    println!("\n(누적: {})", path.display());
    println!("{}", llm.usage().summary());
    Ok(())
}

fn prepare_out(p: &PathBuf) -> Result<PathBuf> {
    std::fs::create_dir_all(p).with_context(|| format!("출력 디렉터리 생성 실패: {}", p.display()))?;
    Ok(p.clone())
}

/// concurrency 만큼 스레드를 묶어 순차 실행(청크 단위 배리어).
fn par_map<T, R, F>(concurrency: usize, items: Vec<T>, f: F) -> Result<Vec<R>>
where
    T: Send,
    R: Send,
    F: Fn(T) -> Result<R> + Sync,
{
    let c = concurrency.max(1);
    let mut out: Vec<R> = Vec::new();
    let mut rest = items;
    while !rest.is_empty() {
        let take = c.min(rest.len());
        let chunk: Vec<T> = rest.drain(..take).collect();
        let results: Vec<Result<R>> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk.into_iter().map(|item| s.spawn(|| f(item))).collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for r in results {
            out.push(r?);
        }
    }
    Ok(out)
}
