# research-loop

Code-Review-Loop(persona 기반 독립리뷰 → discourse 교차검증 → deterministic verdict CLI)을 참고해 시장/경쟁사 리서치 문서화 자동화에 적용하기 위한 리서치/설계 문서.

계기: MangroveCafeOrder 프로젝트에서 국내 카페 POS 경쟁사(페이히어/토스플레이스/나이스포스/티오더/캐시노트)를 여러 라운드에 걸쳐 "다각도로 리서치 → 문서화 → 재조사 → 정정"한 실제 작업을 반복 가능한 파이프라인으로 일반화한 것. 실제로 이 작업 중 발견한 문제들(정량-정성 지표 불일치, 자사 발행 콘텐츠가 검색결과를 장악, 인센티브 리뷰로 인한 신뢰도 오염, 동일 지표의 회차별 수치 불일치)이 설계의 직접적 동기다.

이 저장소는 아직 구현 전 단계이며 아래 두 문서만 포함한다.

## 문서
- [리서치 서베이](docs/research-and-evidence-survey-2026-07-31.md) — 시장조사/경쟁사인텔리전스 자동화 지형, 인용 환각(citation hallucination) 탐지 인접분야, discourse 구조 인접도메인 사례 조사
- [설계 스펙](docs/design-spec.md) — Code-Review-Loop 12단계 파이프라인을 리서치 문서화 도메인(페르소나/spec.toml/deterministic_checks/discourse 규칙/CLI/출력스키마)에 매핑한 설계안

## 원본
아키텍처 원본: Code-Review-Loop (Loop-Suite/codereview-loop). marketing-loop과 동일한 이식 방법론을 따른다.
