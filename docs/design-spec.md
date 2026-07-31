# research-loop 설계 스펙

research-loop는 Code-Review-Loop(Rust 기반 persona-review CLI)의 12단계 파이프라인·페르소나 discourse 구조를 "웹 리서치 기반 시장/경쟁사 조사 문서화" 도메인에 이식하되, 도메인 특성상 불가피한 최소 변경만 반영한 설계다. marketing-loop와 마찬가지로 codereview 원본을 1차 기준으로 삼고, 원본에 없는 확장은 모두 명시적으로 표시한다.

---

## 0. Code-Review-Loop 12단계 파이프라인 대비 research-loop 매핑

| Stage | Module | codereview 원본 | research-loop 치환 |
|---|---|---|---|
| Input normalization / convention injection | input.rs | diff + coding convention | 리서치 대상(`--topic`, 예: "국내 카페 POS 경쟁사") + 기존 문서(있으면) + 톤/포맷 가이드 주입. 문서를 section_id 단위(예: `market-share`, `financials`, `org-culture`)로 정규화 — file:line의 대응물, discourse evidence citation에 필수 |
| Lens selection (3–5) | lens.rs::select_lenses | diff 특성 기반 | `--research-type` 기반 |
| Deterministic vs semantic split | report.rs::deterministic_table | — | 동일 구조 유지 |
| Policy checks / binary verdicts | policy.rs | 코딩 정책 | 출처 표기 누락·인센티브 미표기·데드링크 등 binary 게이트 |
| Per-lens independent review | lens.rs::review_lens | — | 페르소나별 독립 리뷰, 구조 동일 |
| Discourse debate | discourse.rs | AGREE/CHALLENGE/CONNECT/SURFACE | 규칙 대부분 이식, CHALLENGE 조건 재정의(§4) |
| Requirement verification | requirements.rs | PR 요구사항 | 리서치 브리프(다뤄야 할 앵글 목록) 충족 검증 → `coverage_gaps` 산출 |
| Quantitative summarization | quantify.rs | P0=25/P1=12/P2=5/P3=1 | 가중치 숫자 그대로 유지, severity 정의만 도메인 재정의(§7) |
| Prior-run fix check (--prior) | fixcheck.rs + state.rs | FIXED/STILL_OPEN/UNKNOWN | 로직 유지. 단 리서치 도메인에서는 "이전 결론이 최신 근거로 뒤집힌 경우"(예: 티오더-KT 사례)를 별도 `REVERSED`(신규) 상태로 확장 — 원본 3상태에 없던 값, §6 참조 |
| Human-voice rewrite | humanvoice.rs | — | 해당 없음(리서치 문서는 톤 정형화가 목적이 아님) — **비적용, 원본 단계 skip**(marketing-loop과의 명시적 차이) |
| Final report assembly | report.rs | — | 동일 구조 유지 + `citation_status`·`source_diversity` 필드 추가 |

---

## 1. 리서치 도메인 페르소나 (7명)

marketing-loop 컨벤션(실존 인물, 해당 분야의 원칙/저작을 근거로 페르소나 목소리 구성)을 그대로 따른다.

| Lens | Persona (실존) | 근거 | 페르소나 톤 | Tier |
|---|---|---|---|---|
| market_dynamics | Michael Porter | 하버드경영대학원, 『Competitive Strategy』, Five Forces 프레임워크 창시 | 구조적·산업분석 어투, "이게 진짜 구조적 우위인가 일시적 우위인가"를 반복 질문 | 1 |
| financial_forensics | Aswath Damodaran | NYU Stern, "Dean of Valuation", "narrative and numbers must agree" 원칙 | 회의적·정량중심, 서사와 숫자가 어긋나면 즉각 지적, 원출처(감사보고서) 확인 강박 | 1 |
| payments_regulatory_economics | Patrick McKenzie (patio11) | 'Bits about Money' 뉴스레터 저자, 결제산업 구조·규제경제 전문 저술가 | 실무적·수수료구조 해부 어투, "이 비즈니스모델이 규제변화에서 살아남는가"를 반복 질문 | 1 |
| engineering_diligence | Gergely Orosz | 'The Pragmatic Engineer' 뉴스레터, 채용공고·기술블로그 기반 조직진단 방법론 | 채용공고·기술스택 실측 중심, 근거없는 "우리 기술력 최고" 주장에 회의적 | 1 |
| incentive_integrity | Cory Doctorow | "enshittification" 개념 창안, 플랫폼 인센티브 왜곡 비판 저술 | 냉소적·구조비판 어투, "이 후기가 누구의 이익을 위해 존재하는가"를 반복 질문 | 1 |
| org_culture_signal | Adam Grant | Wharton 조직심리학, 『Give and Take』『Originals』 | 데이터 기반 조직행동 해석, 단일 플랫폼 평점을 과신하지 말라고 반복 경고 | 2 |
| closed_platform_ethnography | danah boyd | Microsoft Research/Data & Society, 폐쇄형·알고리즘 플랫폼 연구 방법론 | 방법론적 겸손 강조, "접근 못 한 것"과 "존재하지 않는 것"을 반드시 구분 | 2 |

> **가정:** marketing-loop와 동일하게, 여기서 쓰는 tier=1(필수 5렌즈)/tier=2(보조 2렌즈)는 원본 spec.rs의 `tier: String`(표시용, 선택 로직 비관여)과 다른 의미로 research-loop에서 새로 정의한 필드다. 원본을 엄격히 따르려면 tier는 표시용 라벨로 두고 필수 포함 여부는 별도 `always: bool`로 관리해야 한다(marketing-loop DESIGN 노트 상속).
>
> **가정:** payments_regulatory_economics·incentive_integrity·closed_platform_ethnography 페르소나는 실제 규제기관·법조인이 아니라 "결제경제 해부"·"플랫폼 인센티브 비판"·"폐쇄형 커뮤니티 연구윤리" 철학을 각 렌즈에 근사 매핑한 것이다(claims_compliance가 실제 변호사가 아니었던 marketing-loop과 동일한 유형의 가정). 실제 법적·규제 판정은 페르소나가 아니라 policy.rs의 deterministic 게이트가 담당.

### research-type별 lens selection (pool 7개 중 4–6개)

| --research-type | 선택 렌즈 |
|---|---|
| competitor_landscape | market_dynamics, financial_forensics, engineering_diligence, incentive_integrity |
| financial_diligence | financial_forensics, market_dynamics, payments_regulatory_economics |
| market_sizing | market_dynamics, payments_regulatory_economics, financial_forensics |
| org_and_culture | org_culture_signal, engineering_diligence, incentive_integrity |
| community_sentiment | incentive_integrity, closed_platform_ethnography, org_culture_signal |
| full_deep_dive | market_dynamics, financial_forensics, payments_regulatory_economics, engineering_diligence, incentive_integrity, org_culture_signal, closed_platform_ethnography (전체 7개) |

---

## 2. spec.toml 예시

```toml
[[persona]]
persona_name = "Michael Porter"
persona_voice = "구조적 산업분석 어투. '이것이 지속 가능한 구조적 우위인가, 일시적 프로모션 우위인가'를 반복 질문. Five Forces 관점에서 진입장벽·대체재·구매자교섭력을 짚음."
lens = "market_dynamics"
tier = 1

[[persona]]
persona_name = "Aswath Damodaran"
persona_voice = "회의적·정량중심. 서사(마케팅 카피)와 숫자(감사보고서)가 어긋나는 지점을 즉각 지적. 원출처 확인 없는 재무 주장에 낮은 신뢰도 부여."
lens = "financial_forensics"
tier = 1

[[persona]]
persona_name = "Patrick McKenzie"
persona_voice = "결제산업 구조·수수료 해부 실무 어투. '이 비즈니스 모델이 규제 변화·수수료 인하에도 살아남는가'를 반복 질문."
lens = "payments_regulatory_economics"
tier = 1

[[persona]]
persona_name = "Gergely Orosz"
persona_voice = "채용공고·기술블로그 실측 기반. 근거 없는 '기술력 최고' 마케팅 주장에 회의적, 조직규모·스택을 1차 자료로 재검증."
lens = "engineering_diligence"
tier = 1

[[persona]]
persona_name = "Cory Doctorow"
persona_voice = "냉소적 구조비판. '이 후기가 누구의 이익을 위해 존재하는가'를 반복 질문. 인센티브·리뷰 프로그램의 존재 여부를 항상 먼저 확인."
lens = "incentive_integrity"
tier = 1

[[persona]]
persona_name = "Adam Grant"
persona_voice = "조직심리학 데이터 해석. 단일 리뷰 플랫폼 평점을 과신하지 말라고 경고, 표본크기·응답자편향을 반복 지적."
lens = "org_culture_signal"
tier = 2

[[persona]]
persona_name = "danah boyd"
persona_voice = "방법론적 겸손. '접근하지 못한 것'과 '존재하지 않는 것'을 반드시 구분. 폐쇄형 플랫폼 여론을 단정하는 서술에 제동."
lens = "closed_platform_ethnography"
tier = 2
```

> 위 tier 값의 의미 한계는 §1 가정 항목과 동일.

---

## 3. deterministic_checks 목록

marketing-loop의 "결정론적 검사를 LLM에서 분리" 원칙(bizplan-loop DESIGN.md 항목 11과 동일 근거)을 그대로 따른다. §2(리서치 서베이)에서 관찰한 6개 실패모드를 각각 자동화 가능한 검사로 변환했다.

| check_id | 설명 | 실패모드 대응(서베이 §2) | 로컬 도구/구현 |
|---|---|---|---|
| citation_density_check | 주장 문장 대비 출처링크 비율 | 일반 | 직접구현(문장분리+링크카운트) |
| dead_link_check | 인용 URL 응답코드 확인 | 일반 | linkinator(npm) — 기존도구(marketing-loop과 동일 재사용) |
| source_diversity_check | 출처 도메인 분포, 특히 리서치 대상 기업의 자사 도메인 비중 계산 | "자사 발행 콘텐츠가 검색결과 장악" | 직접구현(도메인 추출+집계) |
| numeric_consistency_check | 동일 entity+metric 조합(예: "티오더 매출")의 수치가 문서 내에서 일관되는지 대조 | "동일 지표의 회차별 수치 불일치" | 직접구현(정규식 기반 entity-metric-value 추출 후 대조) |
| staleness_flag | 문서 내 서술 날짜 vs 인용 기사 발행일 diff 계산, 임계값 초과 시 경고 | "이전 결론이 최신 정보로 뒤집힘" | 직접구현(날짜 파싱+diff) |
| incentive_disclosure_scan | "리뷰 이벤트/제휴/협찬/보상" 키워드 인접 인용문에 인센티브 표기 여부 확인 | "인센티브 리뷰로 인한 신뢰도 오염" | 직접구현(키워드+근접 문맥 스캔) |
| access_limitation_disclosure_check | "확인 안 됨"과 "존재하지 않음"이 실제로 구분되어 서술됐는지 — 접근 시도 기록(예: WebFetch 차단 로그) 존재 여부 대조 | "폐쇄형 플랫폼 접근 불가" | 직접구현(정규식: "확인 안 됨"/"접근 불가"/"단정할 근거 없음" 표현 존재 여부) |
| readability_score | Flesch-Kincaid 등 가독성 지표 | 일반 | textstat(Python) — 기존도구(marketing-loop과 동일 재사용) |
| duplicate_content_check | 회차 간 동일 문단 반복도(재조사 없이 복붙됐는지) | 일반 | simhash — 기존도구(marketing-loop과 동일 재사용) |

**citation_status(인용 환각 대응) 구조적 차이:** marketing-loop의 semgrep 대응 스캐너와 달리, "이 URL이 실제로 이 주장을 뒷받침하는가"는 결정론적으로 완전 자동화할 수 없다(CITETRACER류 프레임워크도 최종적으로 LLM 검증 단계를 둔다 — 리서치 서베이 §5). 따라서 `citation_status`(VERIFIED/UNVERIFIED/STALE/CONTRADICTED)는 deterministic_checks가 아니라 **discourse 라운드에서 페르소나가 원문 대조 후 판정**하는 semantic 영역으로 분리한다.

---

## 4. discourse.rs 이식 판단

| 원본 규칙 | 리서치 도메인 유효성 | 판단 |
|---|---|---|
| 리뷰어 신원 제거, id/file:line/claim/evidence만 남김 | 유효 | 그대로 이식. file:line → section_id:citation_index |
| AGREE는 기존 finding에 없는 새 evidence 인용시만 유효 | 유효 | 그대로 이식. "다른 독립 소스에서 같은 수치·주장을 재확인"할 때만 AGREE 성립(예: 티오더 매출 419억을 서울경제·매일일보 둘 다 인용) |
| CHALLENGE 라운드당 최소1회 강제, 미달시 1회 자동재시도 | 조건부 유효 — 수정필요 | **정량-정성 지표 불일치**(서베이 §2 사례: 앱스토어 평점 vs 정성후기, 잡플래닛 vs 블라인드)가 리서치 도메인의 핵심 CHALLENGE 트리거. 단, "이 정보 오래된 것 같다"는 근거 없는 반박은 CHALLENGE 불인정(SURFACE로 강등) — **유효 CHALLENGE는 반드시 "동일 지표를 다른 방법론/다른 소스로 재측정해 수치 불일치를 제기"하는 경우로 한정**. marketing-loop의 "취향반박 vs 근거기반반박 구분" 원칙과 동일 유형의 보정 |
| CONNECT (다른 렌즈 finding과 연관) | 유효 | 그대로 이식. 예: financial_forensics 렌즈의 적자 발견 ↔ incentive_integrity 렌즈의 "무료 확산 전략" 발견을 연결 |
| SURFACE (새 이슈 제기) | 유효 | 그대로 이식 |

> **가정:** CHALLENGE 조건을 "다른 방법론/소스로 재측정한 불일치만 인정"으로 좁힌 것은 설계판단(원본 README에 근거 없음)이며, marketing-loop이 "취향 vs 근거" 구분을 추가한 것과 같은 유형의 최소보정이다. 검증되지 않은 확장이 아니라, 그대로 이식 시 발생할 결함(근거 없는 트집 반박이 매 라운드 강제 재시도를 유발)에 대한 보정으로 한정한다.

---

## 5. CLI 서브커맨드 매핑

| 서브커맨드 | 코드리뷰 원본 | research-loop 대응 | 1:1 여부 |
|---|---|---|---|
| review | diff/spec/requirements/conventions/deterministic-results → report.md+state.json | 리서치문서/spec/리서치브리프(다룰 앵글 목록, requirements 대응)/톤가이드(conventions 대응)/deterministic-results → report.md+state.json | 1:1, 입력이름만 치환 |
| describe | PR요약: title/summary/walkthrough/labels/can_be_split/TODO스캔 | 문서요약: 핵심발견/커버리지갭/staleness 목록/labels(research-type·리서치대상)/can_be_split(섹션 분리 가능여부)/TODO스캔([확인필요], "추후 업데이트" 표기) | 1:1 |
| improve | before/after 패치제안 | 추가조사 반영 개정 섹션 제안(before/after) | 1:1, "패치"→"개정 섹션"만 치환 |
| ask | 자유질의, ask.md 누적 | 자유질의(예: "이 회사가 PCI-DSS 인증을 받았는지 알아?"), ask.md 누적 | 1:1, 변경없음 |

4개 서브커맨드 모두 구조변경 없이 입력도메인만 치환 가능. 새 서브커맨드 추가하지 않음(marketing-loop과 동일한 최소주의 원칙).

---

## 6. 출력 스키마 (report.md / state.json)

### report.md 필드

- **verdict**(PASS/REVISE — marketing-loop과 동일 가정: 원본 verdict 산출 정확한 수식은 README에 없어 policy-fail override 방식으로 유추, 확실하지 않음)
- **policy checks**(binary pass/fail 목록: 데드링크·인센티브미표기 등)
- **findings**(persona/severity/section위치/claim/evidence)
- **good things**
- **deterministic checks**(check_id별 status/evidence)
- **discourse audit**(라운드별 AGREE/CHALLENGE/CONNECT/SURFACE 로그)
- **requirements verification**(리서치 브리프상 다뤄야 할 앵글 충족여부 → `coverage_gaps` 목록)
- **citation_status 요약**(VERIFIED/UNVERIFIED/STALE/CONTRADICTED 건수 — research-loop 신규, marketing-loop에는 없던 필드)
- **source_diversity 요약**(독립 소스 vs 리서치 대상 자사발행 소스 비율 — research-loop 신규)
- **이전 라운드 대비**(`--prior` 지정 시만, FIXED/STILL_OPEN/UNKNOWN/**REVERSED**(신규, §0 참조) 목록)

> human-voice review 섹션은 **미적용**(§0 참조) — 리서치 문서는 톤 재작성이 목적이 아니므로 marketing-loop의 해당 섹션을 그대로 제거.

### state.json 스키마

> marketing-loop과 동일하게, 원본 state.rs의 `State { round, findings, resolved }` 3필드 구조를 그대로 쓰지 않고 리포트 재구성에 필요한 필드를 확장한 것이다. 원본과 "동일 구조"가 아니라 최소 스냅샷 개념만 참고한 새 설계임을 명시한다.

```json
{
  "run_id": "string",
  "research_type": "competitor_landscape|financial_diligence|market_sizing|org_and_culture|community_sentiment|full_deep_dive",
  "topic": "string",
  "timestamp": "ISO8601",
  "verdict": "PASS|REVISE",
  "score": 0,
  "policy_checks": [{"check_id": "string", "status": "PASS|FAIL", "evidence": "string"}],
  "deterministic_checks": {"check_id": {"status": "PASS|FAIL|WARN", "evidence": "string"}},
  "lens_selected": ["market_dynamics", "financial_forensics"],
  "findings": [{"id": "string", "lens": "string", "persona": "string", "severity": "P0|P1|P2|P3", "section_ref": "section_id:citation_index", "claim": "string", "evidence": "string", "citation_status": "VERIFIED|UNVERIFIED|STALE|CONTRADICTED", "status": "FIXED|STILL_OPEN|UNKNOWN|REVERSED"}],
  "discourse_log": [{"round": 0, "tag": "AGREE|CHALLENGE|CONNECT|SURFACE", "persona": "string", "target_finding_id": "string", "evidence": "string"}],
  "coverage_gaps": ["string"],
  "source_diversity": {"independent_sources": 0, "subject_owned_sources": 0, "ratio": 0.0},
  "good_things": ["string"],
  "prior_ref": "path|null"
}
```

### severity 가중치

quantify.rs 하드코딩값 P0=25/P1=12/P2=5/P3=1 그대로 유지(marketing-loop과 동일, 확장금지). 도메인별 severity 정의만 재해석:

| Severity | 리서치 도메인 정의 |
|---|---|
| P0 | 사실 오류·수치 오염(예: 회계처리방식 변경을 사업위축으로 오독, 재검증 없이 잘못된 재무수치 인용) — 문서 신뢰성 붕괴 리스크 |
| P1 | 출처 편향 미표기(자사발행 콘텐츠를 중립정보처럼 인용, 인센티브 리뷰 미표기) |
| P2 | 커버리지 갭(다뤄야 할 앵글 누락), staleness 미표시 |
| P3 | 사소한 표현·포맷 이슈 |

> **가정:** 이 severity 정의는 설계판단이며 marketing-loop과 마찬가지로 원본 코드도메인 P0-P3의 정확한 정의는 README에 없어 확인 불가 — 숫자 가중치만 원본과 동일 유지.

## 7. 아직 하지 않은 것

- **citation_status 자동판정 파이프라인**: 현재는 discourse 라운드의 페르소나 판정에 전적으로 의존. CITETRACER식 캐스케이딩 검증(캐시조회→URL fetch→커넥터→웹검색)을 결정론적 사전필터로 추가하면 discourse 라운드 부담을 줄일 수 있음(리서치서베이 §5, §다음단계 참조) — 미구현.
- **calibration set**(bizplan-loop DESIGN.md 항목과 동일 성격의 한계): 실제 고품질 리서치 리포트/저품질 리포트 표본으로 루브릭을 보정하는 절차 없이는 severity 임계값이 경험적으로 검증되지 않음.
- **`--watch` 모드**: Klue/Crayon의 실시간 모니터링(Compete Agent) 같은 지속 추적 기능은 본 설계(정적 문서 생성) 범위 밖.
