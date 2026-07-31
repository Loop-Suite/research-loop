# research-loop

A Rust CLI that validates market/competitor research documents through **independent multi-persona review → discourse cross-examination → deterministic verdict**, instead of trusting a single LLM's pass over the text.

The LLM backend is a `claude -p` subprocess (Claude Code CLI) — same approach as [bizplan-loop](https://github.com/Loop-Suite/bizplan-loop). No separate API key required.

## Why this exists

This tool is the generalization of something that actually happened: across many rounds inside the MangroveCafeOrder project, a Korean café-POS competitor research document (5 companies: PayHere, Toss Place, NicePOS, T-order, CashNote) was researched, drafted, re-researched, and corrected over and over. That repeated cycle surfaced concrete failure modes that a single-pass research pipeline does not catch on its own:

- **Quantitative vs. qualitative signals disagreeing** — an app's star rating looked fine while long-form user reviews were scathing, and vice versa.
- **A subject's own marketing content dominating search results**, getting cited as if it were independent evidence.
- **Paid/incentivized reviews contaminating credibility** — one competitor was found to be running a "₩5,000 per review + ₩50,000 per referred install" cash program, meaning positive-sounding blog posts about it could not be taken at face value.
- **The same metric reappearing with different numbers across drafting rounds** (e.g., a competitor's user count cited as both 200K and 300K in different sections, sourced from different secondary articles).
- **A prior conclusion getting reversed by newer evidence** — a "acquisition talks denied" story from mid-2025 turned into a public IP-theft dispute and layoffs by early 2026, and the document had to explicitly mark that its own earlier verdict was overturned, not just updated.

None of these are caught by asking one model to "review this document" once. research-loop encodes the fix as a repeatable pipeline instead of manual vigilance.

> Design rationale, stage-by-stage: **[docs/design-spec.md](docs/design-spec.md)**
> Evidence survey (competitive-intelligence tooling landscape, citation-hallucination-detection adjacent research, and a source-code-level re-verification of several open-source "deep research" agents): **[docs/research-and-evidence-survey-2026-07-31.md](docs/research-and-evidence-survey-2026-07-31.md)**

## Architecture at a glance

```mermaid
flowchart TD
    subgraph Inputs
        DOC["Research document (Markdown)"]
        BRIEF["Brief: angles that must be covered<br/>(optional)"]
        STYLE["Style/tone guide<br/>(optional)"]
        PRIOR["Prior round's state.json<br/>(optional, --prior)"]
    end

    DOC --> NORM["input::normalize<br/>extract sections, citations, word count"]
    BRIEF --> NORM
    STYLE --> NORM

    NORM --> CHECKS["checks.rs<br/>7 deterministic checks"]
    NORM --> SELECT["lens::select_lenses<br/>LLM picks 4-6 of 7 personas"]

    SELECT --> REV1["Persona review #1<br/>(independent, sealed)"]
    SELECT --> REV2["Persona review #2<br/>(independent, sealed)"]
    SELECT --> REVN["Persona review #N<br/>(independent, sealed)"]

    REV1 --> POOL["Findings pool<br/>(reviewer identity stripped)"]
    REV2 --> POOL
    REVN --> POOL

    POOL --> DISCOURSE["discourse::run<br/>AGREE / CHALLENGE / CONNECT / SURFACE<br/>confidence-weighted voting"]
    PRIOR -.-> FIXCHECK["fixcheck::run<br/>FIXED / STILL_OPEN / UNKNOWN / REVERSED"]
    FIXCHECK -.-> DISCOURSE

    DISCOURSE --> COVERAGE["requirements::verify<br/>brief angle coverage → coverage_gaps"]
    CHECKS --> QUANT["quantify::summarize<br/>P0-P3 penalty scoring"]
    COVERAGE --> QUANT
    DISCOURSE --> QUANT

    QUANT --> REPORT["report.md + state.json<br/>verdict: PASS or REVISE"]
```

## Requirements

- Rust 1.70+
- `claude` CLI installed and logged in (pass `--claude-bin` if it's not on `PATH`)

## Build

```bash
cargo build --release   # binary at target/release/research
```

## Subcommands

```bash
# 1) Independent per-persona review + discourse cross-examination (the core pipeline)
research --model sonnet --cheap-model haiku review \
  --spec specs/default.toml --document my-research.md \
  --brief brief.md --out runs/pos

# 2) Summarize the document (key findings, labels, splittability) + scan for "needs verification" markers
research describe --spec specs/default.toml --document my-research.md --out runs/pos

# 3) Propose concrete revisions (things to re-research or correct)
research improve --spec specs/default.toml --document my-research.md --out runs/pos

# 4) Free-form Q&A grounded in the document, appended to ask.md
research ask --spec specs/default.toml --document my-research.md --out runs/pos \
  "Did this company obtain PCI-DSS certification?"
```

## Execution flow of a `review` run

```mermaid
sequenceDiagram
    participant CLI as research review
    participant Spec as spec::load
    participant Input as input::normalize
    participant Checks as checks::run_all
    participant Lens as lens::select_lenses /<br/>review_lens (parallel)
    participant Discourse as discourse::run
    participant Fix as fixcheck::run
    participant Cov as requirements::verify
    participant Report as report::write

    CLI->>Spec: load TOML persona pool
    CLI->>Input: read document, extract sections & citations
    CLI->>Checks: run 7 deterministic checks
    CLI->>Lens: pick 4-6 personas for this document
    par independent, sealed reviews
        Lens->>Lens: Persona A reviews (no visibility into B, C...)
        Lens->>Lens: Persona B reviews
        Lens->>Lens: Persona N reviews
    end
    Lens-->>CLI: findings pool (identities stripped)
    opt --prior <run-dir> supplied
        CLI->>Fix: recheck previously CONFIRMED findings
        Fix-->>CLI: FIXED / STILL_OPEN / UNKNOWN / REVERSED
    end
    CLI->>Discourse: anonymous cross-examination rounds
    Discourse-->>CLI: CONFIRMED / REJECTED / MERGED / UNCERTAIN
    opt --brief supplied
        CLI->>Cov: verify brief angles against confirmed findings
        Cov-->>CLI: coverage_gaps
    end
    CLI->>Report: assemble report.md + state.json
    Report-->>CLI: verdict = PASS or REVISE
```

## Persona pool (7 lenses)

Each lens is voiced by a real analyst/author to suppress sycophancy — the model is told *who* it is arguing as, not just given a topic.

```mermaid
graph TD
    classDef tier1 fill:#2b6cb0,color:#fff,stroke:#1a4971
    classDef tier2 fill:#718096,color:#fff,stroke:#4a5568

    MD["market_dynamics<br/>Michael Porter<br/>structural vs. transient advantage"]:::tier1
    FF["financial_forensics<br/>Aswath Damodaran<br/>narrative must match the numbers"]:::tier1
    PR["payments_regulatory_economics<br/>Patrick McKenzie<br/>does the model survive fee regulation?"]:::tier1
    ED["engineering_diligence<br/>Gergely Orosz<br/>verify via job postings & eng blogs"]:::tier1
    II["incentive_integrity<br/>Cory Doctorow<br/>whose interest does this review serve?"]:::tier1
    OC["org_culture_signal<br/>Adam Grant<br/>don't over-trust a single review platform"]:::tier2
    CP["closed_platform_ethnography<br/>danah boyd<br/>'couldn't access' ≠ 'doesn't exist'"]:::tier2

    subgraph RT["research-type → lens selection"]
        RT1["competitor_landscape"] --> MD & FF & ED & II
        RT2["financial_diligence"] --> FF & MD & PR
        RT3["market_sizing"] --> MD & PR & FF
        RT4["org_and_culture"] --> OC & ED & II
        RT5["community_sentiment"] --> II & CP & OC
        RT6["full_deep_dive"] --> MD & FF & PR & ED & II & OC & CP
    end
```

## Discourse: how findings get confirmed or rejected

Reviewer identity is deliberately stripped before the discourse round — knowing *which* persona raised a finding can make agreement about authority rather than evidence.

```mermaid
stateDiagram-v2
    [*] --> UNRESOLVED: finding raised by a lens
    UNRESOLVED --> CONFIRMED: net confidence-weighted vote >= 0.6
    UNRESOLVED --> REJECTED: net confidence-weighted vote <= -0.6
    UNRESOLVED --> UNCERTAIN: vote in between, rounds exhausted
    UNCERTAIN --> CONFIRMED: later round tips the vote
    UNCERTAIN --> REJECTED: later round tips the vote
    CONFIRMED --> [*]: appears in report.md findings table

    state "Next round only, via --prior" as NextRound {
        CONFIRMED --> FIXED: document addressed it
        CONFIRMED --> STILL_OPEN: unaddressed, re-enters this round's findings
        CONFIRMED --> REVERSED: newer evidence overturns the prior conclusion itself
        CONFIRMED --> UNKNOWN: cannot tell either way
    }
```

`REVERSED` is a research-loop addition that codereview-loop's fixcheck doesn't have — it exists specifically for the "prior conclusion overturned by newer evidence" failure mode (the acquisition-talks-to-IP-dispute example above). `STILL_OPEN` and `REVERSED` both re-enter the current round's findings; only `FIXED` and `UNKNOWN` drop out.

Valid `CHALLENGE` moves are also narrower than in codereview-loop: a challenge only counts if it **re-measures the same metric via a different method or an independent source** and finds a discrepancy. A challenge like "this feels outdated" with no counter-evidence is downgraded to `SURFACE` instead of counting toward the required per-round challenge.

## Deterministic checks (`checks.rs`)

codereview-loop splits this into `policy.rs` + `semgrep.rs`. Research documents have no equivalent of an "auto-fill everything" scanner like semgrep, so there was no reason to keep two modules — they're merged into one.

```mermaid
flowchart LR
    DOC["Normalized document<br/>+ citations list"] --> C1["citation_density_check<br/>claims vs. citation count"]
    DOC --> C2["source_diversity_check<br/>% citations on subject-owned domains"]
    DOC --> C3["numeric_consistency_check<br/>same phrase, conflicting figures"]
    DOC --> C4["staleness_flag<br/>citation year vs. threshold"]
    DOC --> C5["incentive_disclosure_scan<br/>'review event' / 'sponsorship' keywords"]
    DOC --> C6["access_limitation_disclosure_check<br/>honest 'could not verify' phrasing present"]
    DOC --> C7["dead_link_check<br/>real HTTP HEAD/GET via ureq"]

    C1 & C2 & C3 & C4 & C5 & C6 & C7 --> RESULT["PASS / WARN / FAIL / N-A / NOT_CONFIGURED<br/>+ evidence string"]
    RESULT --> REPORTOUT["report.md → Deterministic checks table"]
```

| check | what it does |
|---|---|
| `citation_density_check` | claim-sentence count vs. citation count |
| `source_diversity_check` | share of citations pointing at domains the research *subject* itself owns |
| `numeric_consistency_check` | flags the same short phrase appearing with different numbers in different sections (heuristic, WARN only) |
| `staleness_flag` | flags citation years older than a configurable threshold |
| `incentive_disclosure_scan` | flags "review event / sponsorship / cash reward" language for downstream discourse review |
| `access_limitation_disclosure_check` | checks the document actually says "could not verify" somewhere, rather than staying silent about access limits |
| `dead_link_check` | makes a real HTTP request (via `ureq`) per citation URL; timeouts are `WARN`, not `FAIL` — "unreachable" and "dead" are kept distinct. Skippable with `--skip-link-check` |

## Validated on a real 510-line document

Before shipping, this was run against the actual research document that motivated it (MangroveCafeOrder's POS-competitor research, 510 lines, 97 citations):

- `describe` correctly extracted 10 key findings, all 15 sections, and 1 real "needs verification" marker.
- `review` (2 lenses: `financial_forensics`, `incentive_integrity`) produced `verdict=PASS score=95/100`, and along the way:
  - `numeric_consistency_check` actually caught 8 conflicting figures behind the phrase "operating loss" (₩15.5B / ₩18.6B / ₩49.01B / ₩74.59B / ₩12.8B — each figure was correct for a *different* company/round, but the check correctly flagged that they needed disambiguating).
  - the discourse round surfaced an unverified estimate ("Toss Place: 300K installed stores") that the company's own official figure put at 200K — something a frequency-based "most-repeated-figure-wins" approach (see below) would have missed, since several secondary articles had already repeated the higher, unofficial number.

## It was checked against real open-source competitors, at the source-code level

The evidence survey didn't stop at README pages. Reading actual source files in `assafelovic/gpt-researcher`, `guy-hartstein/company-research-agent`, and `geekan/MetaGPT` corrected one of the survey's own earlier claims: GPT Researcher's README implies "most-frequent-info-wins" voting, but its actual `curator.py` does vector-similarity filtering plus a single LLM ranking pass — no contradiction detection either way. `company-research-agent` (2026, LangGraph + Gemini 2.5 + GPT-5.1, solving the *exact same problem* as research-loop) turned out to be a strictly sequential 6-node pipeline with zero cross-validation anywhere in its source. Full writeup, including the self-correction: [docs/research-and-evidence-survey-2026-07-31.md §8](docs/research-and-evidence-survey-2026-07-31.md).

## Limitations & assumptions

- LLM scoring is not ground-truth fact-checking — it's structured qualitative judgment. `citation_status` (`VERIFIED`/`UNVERIFIED`/`STALE`/`CONTRADICTED`) currently depends entirely on a persona's judgment during the discourse round; CITETRACER-style cascading automatic verification (cache → URL fetch → connector → web search) is not implemented (see docs §7).
- `numeric_consistency_check` uses a word-window regex, not real morphological/entity parsing — expect false positives/negatives. It's `WARN`-only for exactly this reason, never `FAIL`.
- If the generation model and the judge model are the same, it tends to rate its own writing style more favorably. Unlike bizplan-loop, research-loop doesn't yet warn when `--cheap-model` is left unset — worth adding.
- There is no human-voice rewrite stage. Research documents aren't being rewritten for tone, so that codereview-loop stage was dropped entirely rather than adapted (see docs §0).
- `FacTool`'s tool-augmented verification (execute code / re-run math / fetch and diff the actual source) is a promising direction not yet implemented — right now, numeric claims are only checked by an LLM persona's judgment, not by actually re-fetching the cited page and diffing the number against it.

## Lineage

Architecture origin: Code-Review-Loop (`Loop-Suite/codereview-loop`) — independent persona review, anonymized discourse, deterministic verdict. research-loop follows the same porting methodology as `marketing-loop`.
