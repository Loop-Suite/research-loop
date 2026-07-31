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

| 프로젝트 | 규모/주체 | 아키텍처 | 교차검증/discourse 유무 |
|---|---|---|---|
| **GPT Researcher** (assafelovic/gpt-researcher) | ~28,000★, 기여자 240명(2026 중반 기준) — OSS 딥리서치 에이전트 중 가장 널리 채택 | Planner→Executor(크롤러 에이전트)→Publisher 3단. "20개 이상 소스를 긁어 가장 빈도 높은 정보를 채택"하는 **빈도 기반(frequency-based) 검증** | **없음** — 여러 소스가 같은 말을 반복하면 신뢰, 서로 다른 관점이 충돌하며 토론하는 구조가 아님. "다 틀렸을 확률은 낮다"는 통계적 가정에 의존(반박·재측정 절차 부재) |
| **company-research-agent** (guy-hartstein) | LangGraph 기반, Gemini 2.5 Flash + GPT-5.1 | 8노드 **순차 파이프라인**: CompanyAnalyzer/IndustryAnalyzer/FinancialAnalyst/NewsScanner(연구) → Collector/Curator/Briefing/Editor(처리). Curator가 Tavily 관련성 점수(임계값 0.4)로 필터링 | **명시적으로 없음(확인됨)** — 각 노드가 일방향으로 실행되고, Curator의 중복제거·URL dedup만 있을 뿐 사실검증이나 페르소나 간 반박 단계는 문서화돼 있지 않음. **research-loop과 도메인이 가장 가깝고 저장소도 신선한(2026년) 프로젝트인데도 discourse 구조가 전무하다는 점이 가장 직접적인 반증 사례** |
| **DeerFlow** (ByteDance) | 대기업 OSS | 계획-실행 루프 기반 자율 조사 | 확인 안 됨(개요만 확인, 세부 미조사) |
| **open_deep_research** (LangChain) | Deep Research Bench #6위(종합점수 0.4344) | LangGraph 기반 리서치 그래프 | 확인 안 됨 |
| **MetaGPT** | ~50,000★, "Code = SOP(Team)" | 요구사항 입력 → 유저스토리/**경쟁분석**/데이터구조/API스펙/코드 출력까지 SOP화된 역할 분업 | 역할별 산출물 검토는 있으나 독립판정→익명토론 구조는 확인 안 됨 |
| **Loki** (오픈소스 사실검증 툴) | 학술 도구 | 5단계: 주장식별→체크워딩(check-worthiness)판정→근거조회 쿼리생성→근거조회(Serper API)→검증 | discourse 아님, 단일 선형 파이프라인. 다만 "체크워딩 판정" 단계는 research-loop의 citation_density_check가 하지 못하는 "이 주장이 애초에 검증할 가치가 있는가"를 사전 필터링하는 아이디어로 참고 가치 있음 |

### 종합 결론 (기존 §7과의 관계)

- **§7의 결론이 OSS 딥리서치 생태계에서도 그대로 재확인됨**: 어떤 프로젝트도 "독립 페르소나 리뷰 → discourse 교차검증 → 결정론적 verdict" 3단 구조를 갖추지 않았다. 가장 근접한 것은 GPT Researcher의 "빈도 기반 검증"인데, 이는 능동적 반박(CHALLENGE)이 없는 수동적 합의 탐지일 뿐이다 — 여러 출처가 같은 오류를 베껴 쓴 경우(예: 이번 POS 리서치에서 실제로 발견한 "토스플레이스 30만곳" 같은 추정치가 여러 매체에 반복 인용된 사례)를 걸러내지 못한다는 구조적 약점이 있다.
- **company-research-agent가 가장 중요한 반증 사례**: research-loop과 완전히 동일한 문제(기업 리서치 자동화)를 2026년 최신 스택(LangGraph+Gemini 2.5+GPT-5.1)으로 풀면서도 순차 파이프라인+관련성 필터링에 그친다. 이는 "discourse 구조가 아직 이 시장에 없다"는 §7 결론이 낡은 조사가 아니라 현재도 유효하다는 실측 근거다.
- **Loki의 check-worthiness 사전필터**는 checks.rs의 `citation_density_check`(밀도만 측정, 주장의 검증가치는 안 봄)를 보강할 아이디어로 백로그에 추가할 만하다 — 미구현.

### 실사용 스모크테스트로 얻은 부가 확인

이 조사 직후 research-loop CLI를 실제로 빌드해 MangroveCafeOrder의 POS 경쟁사 리서치 문서(510줄, 인용 97건)에 `review`를 돌렸다. `numeric_consistency_check`가 "영업손실" 문구에 8종의 서로 다른 수치(155억/186억/490.1억/745.9억/128억 등, 각기 다른 회사·회차를 가리킴에도 동일 문구로 묶여 탐지됨)를 실제로 잡아냈고, discourse 라운드에서 "토스플레이스 30만곳(자사 발표는 20만곳, 30만은 추정)" 같은 미검증 추정치를 SURFACE로 제기했다 — GPT Researcher식 "빈도 기반 검증"이었다면 여러 2차 매체가 반복 인용한 "30만곳"을 그대로 신뢰했을 사례다. **이는 위 종합 결론(빈도 기반 검증의 구조적 약점)을 실제 산출물로 재확인한 것**이다.
