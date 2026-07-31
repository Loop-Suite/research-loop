# research-loop 리서치 서베이

## 1. 개요

Code-Review-Loop의 핵심 구조는 **① 페르소나별 독립 리뷰 → ② discourse를 통한 익명 교차토론(강제 CHALLENGE) → ③ 결정론적 verdict 산출**이다. 이 구조를 "웹 리서치 기반 시장/경쟁사 조사 문서화" 도메인(research-loop)에 이식할 수 있는지 판단하기 위해 (A) GitHub의 시장조사/경쟁정보(CI) 자동화 스킬 생태계, (B) 상용 경쟁정보(CI) 플랫폼의 아키텍처, (C) 리서치 문서 고유의 실패모드(인용 환각·수치 불일치)를 다루는 인접 연구를 조사했다. marketing-loop의 discourse 인접도메인 조사(fact-check/법률/동료평가)는 도메인에 종속되지 않는 내용이므로 그대로 상속하고, 본 문서에서는 리서치 도메인 고유 항목만 추가 조사한다.

## 2. 동기: 실제 리서치 작업에서 관찰된 실패모드

이 설계는 MangroveCafeOrder 프로젝트의 POS 경쟁사 리서치(여러 라운드에 걸친 실제 작업)에서 반복 관찰된 아래 현상들을 직접적인 근거로 삼는다.

| 현상 | 실제 사례 |
|---|---|
| 정량-정성 지표 불일치 | 페이히어 앱스토어 평점(4.0/245건)이 정성 후기(클리앙, 부정적)보다 높게 나옴. 잡플래닛(2.7)과 블라인드(3.5) 간 0.8점 격차 |
| 자사 발행 콘텐츠가 검색결과 장악 | "카페 포스기 추천" 검색 시 토스플레이스 자체 블로그("사장님 스토리")가 반복 상위 노출 — 독립 소스와 구분 안 하면 마케팅 카피를 중립 정보로 오인 |
| 인센티브 리뷰로 인한 신뢰도 오염 | 토스플레이스가 "후기 작성 5천원 + 추천설치 5만원" 현금보상 프로그램을 운영 중임을 확인 — 인용된 긍정 후기 상당수가 금전적 유인 하에 작성됐을 가능성 |
| 동일 지표의 회차별 수치 불일치 | 티오더 연동 가능 POS 개수가 출처마다 "25개"~"50개+"로 상이. 티오더 매출이 2023년 587억 vs 2025년 419억으로 보였으나 실제로는 회계처리방식 변경이 원인(회사 자체 위축 아님) — 재조사 없이는 오독 |
| 이전 결론이 최신 정보로 뒤집힘 | 티오더-KT 관계를 "매각검토설, 확정된 바 없음"으로 기록했으나 3개월 뒤 재조사 결과 공개 기술탈취 분쟁·구조조정으로 악화된 사실 확인 — 리서치 문서는 "최신 회차가 항상 최선"이 아니라 근거 날짜를 명시해야 함 |
| 폐쇄형 플랫폼 접근 불가 | 국내 최대 자영업자 커뮤니티(네이버 카페)가 로그인 기반이라 검색엔진에 색인되지 않음 — "확인 안 됨"과 "존재하지 않음"을 구분해서 기록해야 함 |

이 표의 각 행은 §5(discourse 이식) 및 설계스펙의 deterministic_checks·severity 정의에 직접 반영된다.

## 3. GitHub 시장조사/경쟁정보(CI) 자동화 스킬 생태계

| 레포 | 특징 | discourse 단계 |
|---|---|---|
| ferdinandobons/startup-skill | 스타트업 검증·경쟁정보·기획 AI 에이전트 스킬 | 확인 불가 |
| phuryn/pm-skills | PM Skills Marketplace, 100+ 스킬(시장조사: 페르소나/세그멘테이션/여정맵/시장규모/경쟁분석 포함) | 확인 불가 |
| Imbad0202/academic-research-skills | Deep Research 13-agent 리서치팀, Socratic 가이드 모드, PRISMA 체계적 리뷰, Semantic Scholar API 검증 | 부분적 — cross-model 이중검증(DA) 옵션 언급되나 강제 CHALLENGE 아님 |

결론: SKILL.md 형식 시장조사 스킬은 존재하나, Code-Review-Loop식 강제 discourse 교차검증을 갖춘 스킬은 **확인 불가**(marketing-loop 조사와 동일한 결론).

## 4. 상용 경쟁정보(CI) 플랫폼 아키텍처

| 도구 | 구조 | 가격대 |
|---|---|---|
| Klue | 웹 모니터링 + AI 큐레이션 + "Compete Agent"(영업통화 실시간 경쟁사 언급 탐지) | ~$20K-40K/yr |
| Crayon | 엔터프라이즈 모니터링 + battlecard + 필드 인텔리전스 통합 | ~$20K-40K/yr |
| Kompyte | 웹/디지털 추적 자동화 중심(2014년부터, 現 Semrush 편입) | ~$300/yr~ |

**아키텍처 관찰**: 3사 모두 "① 데이터 수집(웹모니터링) → ② AI 큐레이션/요약(단일 레이어)" 2단 구조다. 페르소나 독립 리뷰나 교차토론(discourse) 구조를 뒷받침하는 근거는 어디에도 없다 — marketing-loop 조사에서 확인한 "상용 도구는 대부분 생성/큐레이션 2단, 3단 discourse 구조 선례 없음" 결론과 정확히 일치한다.

**리서치 기법으로서의 참고 가치**: CI 업계 실무에서 언급되는 근거수집 기법(특허 분석으로 혁신 패턴 추적, **채용공고 분석으로 경쟁사의 목표 산업·성장 우선순위·제품 방향 추론**)은 이번 POS 리서치에서 실제로 사용한 방법(원티드/랠릿 채용공고로 기술스택·조직규모 추정)과 정확히 일치 — 업계 표준 기법으로 확인됨.

## 5. 리서치 문서 고유 실패모드: 인용 환각(citation hallucination) 탐지

marketing-loop 조사범위에는 없던 항목으로, 리서치 문서 자동화의 핵심 리스크이므로 별도 조사.

- **"Source or It Didn't Happen" (CITETRACER, arXiv:2605.08583)** — 인용 환각 탐지를 12-코드 분류체계(REAL/POTENTIAL/HALLUCINATED)로 재정의하고, PDF/BibTeX에서 구조화된 인용을 추출한 뒤 캐시조회→URL fetch→scholar 커넥터→웹검색 순으로 근거를 계단식(cascading)으로 검증하는 멀티에이전트 탐지기를 제시. **재사용 포인트**: "검증 안 됨"과 "허위로 확인됨"을 별도 등급으로 분리하는 방식 — 본 설계의 `citation_status`(VERIFIED/UNVERIFIED/STALE/CONTRADICTED) 필드에 직접 반영.
- **"Detecting and Correcting Reference Hallucinations in Commercial LLMs and Deep Research Agents" (arXiv:2604.03173)** — 상용 딥리서치 에이전트에서도 참조 환각이 발생함을 실측. 리서치 문서 자동화가 일반 텍스트 생성보다 인용 정확성 요구가 높다는 근거.
- **Academic Paper Reviewer 7-agent 프레임워크** (다중관점 동료평가: EIC + 동적 리뷰어 3 + Devil's Advocate, concession threshold 프로토콜) — 페르소나 다양성 + 의도적 반박역할(Devil's Advocate) 조합이 discourse.rs의 강제 CHALLENGE와 목적이 같음. 단, 코드 수준 갱신규칙(임계값 등)은 원문에서 확인 안 됨.
- **MAD-Fact (arXiv:2510.22967)** — 장문 사실성(long-form factuality) 평가를 위한 멀티에이전트 토론 프레임워크. 리서치 문서처럼 여러 개별 주장(claim)이 뒤섞인 장문 텍스트의 사실성 평가에 특화 — 리서치 리포트 채점에 marketing-loop보다 더 직접적으로 대응하는 선례.

**종합**: discourse 교차검증 구조 자체는 marketing-loop가 이미 확인한 인접도메인(fact-check/법률/동료평가) 사례를 그대로 상속하되, "인용 환각 탐지"라는 리서치 도메인 고유의 하위문제는 CITETRACER의 등급분류(REAL/POTENTIAL/HALLUCINATED)를 `citation_status` 필드로, MAD-Fact의 장문 사실성 평가 구조를 discourse 라운드 설계의 참고로 추가 반영한다.

## 6. discourse(독립판정→교차토론→합의) 인접도메인 사례 — marketing-loop 조사 상속

marketing-loop의 §4(fact-check/저널리즘, 법률검토, 학술 동료평가, HAJailBench 종료조건, 법률 MAD 3-Ply 구조)는 도메인 비종속적 조사이므로 그대로 상속한다. 요약:

- Code-Review-Loop 수준(강제 CHALLENGE·익명화·file:line 신규증거 필수)으로 규칙이 코드화된 사례는 어느 인접도메인에도 없음.
- 가장 근접: HAJailBench(종료조건 정량화 — 유사도 임계값, risk band 수렴), 법률 MAD(arXiv:2606.30906, 모델호출횟수까지 명시된 3-Ply 구조).
- 본 조사에서 추가 확인한 MAD-Fact(장문 사실성)와 CITETRACER(인용 환각 등급분류)를 리서치 도메인 전용 보강 근거로 §5에 추가.

## 7. 종합 결론

- 시장조사/CI 자동화 생태계(스킬·상용도구 모두)에 "독립 페르소나 리뷰 → discourse 교차검증 → 결정론적 verdict" 3단 구조 선례는 **확인되지 않는다** — marketing-loop 조사와 동일한 결론이 CI 도메인에서도 재확인됨.
- 리서치 도메인 고유의 차별화 지점은 **인용 환각 탐지**(CITETRACER)와 **장문 사실성 평가**(MAD-Fact)로, 이 둘을 marketing-loop에는 없던 `citation_status` 필드와 discourse 라운드 설계에 반영한다.
- CI 업계의 실무 근거수집 기법(채용공고·특허 분석)은 이번 POS 리서치에서 실제 사용한 방법과 일치 — lens 설계(§공식 설계스펙 참조)에 "engineering_diligence" 페르소나로 반영.
- 실제 리서치 작업에서 관찰된 6가지 실패모드(§2)는 각각 deterministic_checks 또는 discourse CHALLENGE 조건으로 1:1 대응시켰다(설계스펙 §3, §4 참조).

### 다음 단계 제안

- CITETRACER의 캐스케이딩 검증 순서(캐시조회→URL fetch→커넥터→웹검색)를 `citation_status` 판정 파이프라인의 1차 참고 템플릿으로 검토.
- MAD-Fact의 장문 사실성 평가 세부 알고리즘(claim decomposition 방식)이 공개돼 있다면 discourse 라운드 설계에 추가 반영할 가치가 있음 — 이번 조사에서는 개요만 확인, 세부 재조사 필요.
- 상용 CI 도구(Klue/Crayon)의 "Compete Agent" 실시간 모니터링 기능은 본 설계(정적 문서 생성)의 범위 밖이나, 향후 `--watch` 모드 확장 시 참고 가능.

## 8. 후속 조사: OSS 딥리서치/기업조사 에이전트 아키텍처 (2026-07-31, 구현 이후 추가조사)

Rust CLI 구현을 마친 뒤, "독립 페르소나 → discourse → 결정론적 verdict" 구조가 실제로 차별화 지점인지 오픈소스 딥리서치 생태계에서 한 번 더 검증했다. citation-hallucination 인접분야(§5)보다 더 직접적으로 "경쟁사/기업 리서치 자동화"라는 research-loop과 동일한 문제를 푸는 OSS 프로젝트들을 조사 대상으로 삼았다.

### 조사 대상과 아키텍처

최초 조사는 README/랜딩페이지 기반이었다. **아래는 실제 소스 파일을 직접 읽어 재검증한 결과이며, 그 과정에서 README 서술과 실제 코드가 어긋나는 지점을 하나 발견해 정정했다** — 이 정정 자체가 "1차 자료로 재검증하라"는 research-loop의 engineering_diligence 렌즈 취지를 스스로 실천한 사례다.

| 프로젝트 | 규모/주체 | 아키텍처(코드 근거) | 교차검증/discourse 유무 |
|---|---|---|---|
| **GPT Researcher** (assafelovic/gpt-researcher) | ~28,000★, 기여자 240명(2026 중반 기준) — OSS 딥리서치 에이전트 중 가장 널리 채택 | **[정정]** README는 "20개 이상 소스 중 가장 빈도 높은 정보 채택"이라 서술하지만, 실제 `skills/researcher.py`·`skills/curator.py` 코드를 읽으면 다르다 — `_get_context_by_web_search()`가 서브쿼리를 병렬 수집한 뒤 `get_similar_content_by_query()`로 **벡터 임베딩 의미유사도** 기반 필터링을 하고, `curate_sources()`가 LLM 1회 호출로 관련성·신뢰성·정확성 3기준을 매겨 상위 N개만 채택한다. "빈도"가 아니라 **의미유사도 + 단일 LLM 순위매김** | **없음(코드로 확인)** — `curator.py`는 소스를 랭킹해 상위 N개를 반환할 뿐, 서로 모순되는 소스를 탐지·해소하는 로직이 없다("오류 시 원본 데이터 그대로 반환"). 여러 소스가 같은 오류를 베껴 써도 걸러낼 방법이 없다 |
| **company-research-agent** (guy-hartstein) | LangGraph 기반, Gemini 2.5 Flash + GPT-5.1 | `backend/nodes/` 실제 파일: `grounding.py`(대상 회사 웹사이트 Tavily crawl, 최대 50페이지 — "그라운딩"이지만 사실검증이 아니라 원본자료 수집일 뿐), `collector.py`, `curator.py`(관련성 랭킹), `enricher.py`(URL별 raw content 채움, 병렬배치 처리), `briefing.py`(Gemini로 카테고리 요약), `editor.py`(GPT-5.1로 최종본 컴파일) | **명시적으로 없음(소스 3개 파일을 직접 읽어 확인)** — grounding.py·enricher.py·curator.py 어디에도 모순 탐지·교차검증 로직이 없다. **research-loop과 도메인이 완전히 같고 스택도 최신(2026)인 프로젝트조차 discourse 구조가 코드 수준에서 전무함을 직접 확인** |
| **MetaGPT** | ~50,000★, "Code = SOP(Team)" | `actions/write_prd.py`의 `_save_competitive_analysis()`를 직접 읽음 — PRD 생성 시 함께 산출된 "COMPETITIVE_QUADRANT_CHART" 데이터를 Mermaid로 변환해 SVG로 렌더링만 함 | **없음(코드로 확인)** — 경쟁분석은 PRD 생성 단계의 **단일 LLM 호출 부산물**이며, 별도 검증·교차확인 단계 자체가 없다. 마케팅 문구("SOP화된 팀")보다 실제로는 훨씬 단순한 단일 패스 |
| **FacTool** (GAIR-NLP, 신규 추가) | 학술 OSS | 지식기반QA/코드/수학/과학문헌 4개 도메인별로 **도구증강(tool-augmented)** 검증 — QA는 Serper 다중소스, 코드는 실제 실행 결과, 수학은 Python 실행 재검증, 논문은 원문 대조. claim-level과 response-level 이중 점수 | discourse는 없으나 **"LLM 판단이 아니라 실제 실행/원문 대조로 검증"하는 방향성**이 research-loop의 `citation_status`(현재는 discourse 페르소나 판정에 의존)를 코드 실행 기반으로 보강할 여지를 시사 — 예: 수치 주장은 인용 URL을 실제로 fetch해 문자열 대조하는 결정론적 보조 검사 추가 가능 |
| **DeerFlow** (ByteDance) / **open_deep_research** (LangChain, Deep Research Bench #6위) | 대기업/커뮤니티 OSS | 계획-실행 루프형 리서치 그래프 | 소스코드 재검증 못함(개요만, 시간 제약) — 이번 라운드에서 검증 수준이 다른 항목들과 다르다는 점을 명시 |
| **Loki** (오픈소스 사실검증 툴) | 학술 도구 | 5단계: 주장식별→체크워딩(check-worthiness)판정→근거조회 쿼리생성→근거조회(Serper API)→검증 | discourse 아님, 단일 선형 파이프라인. "체크워딩 판정" 단계는 참고 가치 있음(아래) |

### 종합 결론 (기존 §7과의 관계, 소스코드 재검증 후 갱신)

- **§7의 결론이 코드 수준에서 재확인됨**: GPT Researcher(curator.py)·company-research-agent(3개 노드 파일)·MetaGPT(write_prd.py) 세 프로젝트 모두 README 설명 여부와 무관하게 실제 소스에 모순탐지·교차검증 로직이 없다. "독립 페르소나 리뷰 → discourse → 결정론적 verdict" 3단 구조는 여전히 확인되지 않는다.
- **방법론적 자기교정**: GPT Researcher를 "빈도 기반"이라고 최초 기술했던 것은 README 문구를 그대로 옮긴 것이었고, 실제 코드는 "의미유사도 벡터 필터링 + 단일 LLM 랭킹"이었다. 이 차이는 사소하지 않다 — 전자는 "다수결", 후자는 "한 번의 LLM 판단"에 가까워 오류 발생 지점이 다르다. **README만 보고 아키텍처를 단정하면 안 된다는 것을 이 조사 스스로 실증**했다.
- **company-research-agent가 가장 중요한 반증 사례**: research-loop과 완전히 동일한 문제(기업 리서치 자동화)를 2026년 최신 스택으로 풀면서도, grounding→collect→curate→enrich→brief→edit 6단계 어디에도 반박·재측정 절차가 없음을 소스 레벨에서 확인했다.
- **FacTool의 도구증강 검증**은 새로운 보강 아이디어다 — discourse(정성 판단)와 별개로, 수치 주장에 대해서는 인용 URL을 실제 fetch해 문자열/숫자 대조하는 **결정론적 2차 검증**을 checks.rs에 추가할 여지가 있다(미구현, 백로그).
- **Loki의 check-worthiness 사전필터**는 checks.rs의 `citation_density_check`(밀도만 측정, 주장의 검증가치는 안 봄)를 보강할 아이디어로 백로그에 추가할 만하다 — 미구현.

### 실사용 스모크테스트로 얻은 부가 확인

이 조사 직후 research-loop CLI를 실제로 빌드해 MangroveCafeOrder의 POS 경쟁사 리서치 문서(510줄, 인용 97건)에 `review`를 돌렸다. `numeric_consistency_check`가 "영업손실" 문구에 8종의 서로 다른 수치(155억/186억/490.1억/745.9억/128억 등, 각기 다른 회사·회차를 가리킴에도 동일 문구로 묶여 탐지됨)를 실제로 잡아냈고, discourse 라운드에서 "토스플레이스 30만곳(자사 발표는 20만곳, 30만은 추정)" 같은 미검증 추정치를 SURFACE로 제기했다. 위에서 재검증한 세 프로젝트(GPT Researcher의 벡터유사도+단일랭킹, company-research-agent의 순차 파이프라인, MetaGPT의 단일 LLM 부산물) 중 무엇을 썼어도 이런 반박·재측정 단계 없이 "여러 2차 매체가 반복 인용한 30만곳"을 그대로 채택했을 것이다 — **이는 discourse 구조의 실효성을 실제 산출물로 재확인한 것**이다.
