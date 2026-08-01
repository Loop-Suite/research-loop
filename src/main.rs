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

/// #9: 프롬프트 JSON 스키마/지시문이 구조적으로 바뀔 때만 수동으로 올리는 버전 문자열.
/// state.json에 기록되어, 과거 라운드와 비교할 때 "프롬프트 자체가 달라졌는지"를 구분하는 데 쓴다.
const PROMPT_VERSION: &str = "1";

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
    match real_main() {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("에러: {e:#}");
            std::process::exit(1);
        }
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

/// PASS=0, REVISE=1(#12) — review 서브커맨드만 verdict 기반 종료 코드를 갖는다. 나머지 서브커맨드는
/// 정상 완료 시 항상 0(에러는 이 함수가 아니라 main()의 Err 분기에서 exit(1)로 처리됨).
fn real_main() -> Result<i32> {
    let cli = Cli::parse();
    let (llm, cheap_llm) = build_llm(&cli)?;

    match &cli.cmd {
        Cmd::Review { spec, document, brief, style, deterministic_results, lenses, out, concurrency, max_rounds, prior, as_of_year, skip_link_check } => {
            run_review(&llm, &cheap_llm, spec, document, brief, style, deterministic_results, lenses, out, *concurrency, *max_rounds, prior, *as_of_year, *skip_link_check)
        }
        Cmd::Describe { spec, document, brief, style, out } => {
            run_describe(&llm, spec, document, brief, style, out)?;
            Ok(0)
        }
        Cmd::Improve { spec, document, brief, style, out } => {
            run_improve(&llm, spec, document, brief, style, out)?;
            Ok(0)
        }
        Cmd::Ask { spec, document, brief, style, out, question } => {
            run_ask(&llm, spec, document, brief, style, out, question)?;
            Ok(0)
        }
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
) -> Result<i32> {
    let started_at = state::unix_ts();
    let sp = Spec::load(spec_path)?;
    let inp = input::normalize(document_path, brief_path, style_path, deterministic_results_path)?;
    const DOC_WARN_CHARS: usize = 300_000;
    if inp.document.len() > DOC_WARN_CHARS {
        eprintln!(
            "경고: 문서가 {}자로 큼 — 렌즈별 리뷰·discourse·커버리지 호출마다 전체가 재전송되어 토큰 비용이 커짐",
            inp.document.len()
        );
    }

    let as_of = as_of_year.unwrap_or_else(|| default_as_of_year(&inp.document));

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
        discourse::run(llm, &sp, &mut findings, max_rounds, concurrency)?
    };

    // #4: citation_status는 LLM 자기판정을 그대로 신뢰하지 않고, 코드가 실제로 HTTP 재요청 +
    // 인용 문구 대조를 수행해 UNFETCHED/FETCH_FAILED/QUOTE_MATCHED/QUOTE_NOT_FOUND로 덮어쓴다.
    // LLM이 낸 원래 값은 finding.llm_citation_status에 참고용으로만 남는다.
    checks::verify_citations(&inp, &mut findings, skip_link_check);

    // #7: --prior 재검사 결과를 FIXED(닫음)/STILL_OPEN(유지)/REVERSED(신규 고위험 finding)/
    // UNKNOWN(유지+human review 플래그) 4갈래로 명시적으로 분기한다. 예전엔 STILL_OPEN/REVERSED만
    // 처리하고 나머지(특히 UNKNOWN)는 findings/score에서 조용히 사라졌다 — "확인 불가"를 "해결됨"처럼
    // 취급하는 것은 안전성 문제라 UNKNOWN도 반드시 남기고 사람 확인을 요구하도록 바꿨다.
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
            let Some(orig) = prior_confirmed.iter().find(|f| f.id == fr.finding_id) else {
                continue;
            };
            match fr.status.as_str() {
                "FIXED" => {
                    // 닫음 — findings/resolved에 다시 넣지 않는다. 리포트/점수에서 자연스럽게 빠진다.
                }
                "STILL_OPEN" => {
                    findings.push(orig.clone());
                    resolved.insert(
                        orig.id.clone(),
                        discourse::Resolution {
                            finding_id: orig.id.clone(),
                            status: "CONFIRMED".to_string(),
                            merged_into: String::new(),
                            reason: format!("이전 라운드 대비 STILL_OPEN: {}", fr.evidence),
                            needs_human_review: false,
                        },
                    );
                }
                "REVERSED" => {
                    // 이전 결론 자체가 뒤집힌 경우 — 기존 finding을 그대로 재사용하지 않고,
                    // 신규 고위험(P0) finding으로 승격해 별도 id로 남긴다.
                    let mut reversed = orig.clone();
                    reversed.id = format!("{}-reversed-r{}", orig.id, round);
                    reversed.severity = "P0".to_string();
                    reversed.evidence = format!("[REVERSED] 이전 결론이 최신 근거로 뒤집힘: {}", fr.evidence);
                    findings.push(reversed.clone());
                    resolved.insert(
                        reversed.id.clone(),
                        discourse::Resolution {
                            finding_id: reversed.id.clone(),
                            status: "CONFIRMED".to_string(),
                            merged_into: String::new(),
                            reason: format!("이전 라운드 대비 REVERSED(신규 고위험 finding으로 승격): {}", fr.evidence),
                            needs_human_review: true,
                        },
                    );
                }
                "UNKNOWN" => {
                    // 확인 불가 — FIXED처럼 조용히 지우지 않고 유지하되, human review 필요 플래그를 세운다.
                    findings.push(orig.clone());
                    resolved.insert(
                        orig.id.clone(),
                        discourse::Resolution {
                            finding_id: orig.id.clone(),
                            status: "CONFIRMED".to_string(),
                            merged_into: String::new(),
                            reason: format!("이전 라운드 대비 확인 불가(UNKNOWN) — 자동 해제하지 않음, 인간 확인 필요: {}", fr.evidence),
                            needs_human_review: true,
                        },
                    );
                }
                other => {
                    eprintln!("경고: fix check가 알 수 없는 status \"{other}\"를 반환함(finding {})", fr.finding_id);
                }
            }
        }
    }

    // #6: --deterministic-results로 외부 결과가 주어졌으면(inp.deterministic_results가 이미
    // input::normalize 단계에서 파싱해 채워둠) 그 외부 결과를 스키마 검증 후 그대로 쓰고, 내부
    // checks::run_all()을 다시 돌려서 덮어쓰지 않는다 — 예전엔 외부 결과가 완전히 무시되고 항상
    // 내부 재실행 결과만 report/verdict에 반영됐다.
    let checks_results: Vec<checks::CheckResult> = match &inp.deterministic_results {
        Some(external) => checks::from_json(external).context("--deterministic-results 스키마 검증 실패")?,
        None => checks::run_all(&sp, &inp, &checks::CheckOptions { as_of_year: as_of, skip_link_check }),
    };
    // 이번 라운드에 실제로 반영된 checks_results를 그대로 스냅샷해둔다 — 다음 실행에서
    // `--deterministic-results runs/deterministic-results.json`으로 재사용하거나(외부 스캐너 대체),
    // 결과를 그대로 감사할 수 있게(#6). from_json이 그대로 다시 읽을 수 있는 형식이다.
    let det_results_path = out_dir.join("deterministic-results.json");
    std::fs::write(&det_results_path, serde_json::to_string_pretty(&checks::to_json(&checks_results))?)
        .with_context(|| format!("{} 쓰기 실패", det_results_path.display()))?;

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

    // #9: RunManifest 필드(재현·감사용) — 입력/spec fingerprint, 모델/provider, 시간, 비용, 프롬프트 버전.
    let provider_label = match &llm.provider {
        crate::llm::Provider::ClaudeCli { .. } => "claude-cli",
        crate::llm::Provider::OpenRouter { .. } => "openrouter",
    };
    let usage = llm.usage();
    state::write(
        &out_dir,
        &state::State {
            round,
            findings: findings.clone(),
            resolved: resolved.clone(),
            input_hash: state::fingerprint_str(&inp.document),
            spec_hash: state::fingerprint_str(&serde_json::to_string(&sp).unwrap_or_default()),
            model_id: llm.model.clone().unwrap_or_default(),
            provider: provider_label.to_string(),
            started_at,
            completed_at: state::unix_ts(),
            cost_usd: usage.cost_usd,
            prompt_version: PROMPT_VERSION.to_string(),
        },
    )?;

    println!("\n종료 — verdict={} score={}/100 coverage_gaps={}", quant.verdict, quant.score, quant.coverage_gap_count);
    println!("리포트: {}", path.display());
    println!("다음 라운드: --prior {}", out_dir.display());
    println!("{}", llm.usage().summary());

    // #12: REVISE도 exit 0으로 끝나 CI 게이트로 못 쓰던 문제 — PASS만 0, 그 외(REVISE)는 1.
    Ok(if quant.verdict == "PASS" { 0 } else { 1 })
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
/// discourse.rs의 렌즈별 독립 critic 호출(#1)도 이 헬퍼를 재사용한다.
pub(crate) fn par_map<T, R, F>(concurrency: usize, items: Vec<T>, f: F) -> Result<Vec<R>>
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
