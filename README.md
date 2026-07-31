# research-loop

시장/경쟁사 리서치 문서를 **다각도(7페르소나) 독립 리뷰 → discourse 교차검증 → 결정론적 verdict** 로 검증하는 Rust CLI.
LLM 백엔드는 Claude Code CLI(`claude -p`) 서브프로세스(bizplan-loop과 동일 방식). 별도 API 키 불필요.

계기: MangroveCafeOrder 프로젝트에서 국내 카페 POS 경쟁사(페이히어/토스플레이스/나이스포스/티오더/캐시노트)를 여러 라운드에 걸쳐 "다각도로 리서치 → 문서화 → 재조사 → 정정"한 실제 작업을 반복 가능한 파이프라인으로 일반화한 것. 그 과정에서 발견한 문제들(정량-정성 지표 불일치, 자사 발행 콘텐츠가 검색결과를 장악, 인센티브 리뷰로 인한 신뢰도 오염, 동일 지표의 회차별 수치 불일치, 이전 결론이 최신 근거로 뒤집힘)이 설계의 직접적 동기다.

> 단계별 설계 근거: **[docs/design-spec.md](docs/design-spec.md)**
> 리서치 서베이(경쟁사 CI 자동화 지형, 인용 환각 탐지 인접분야): **[docs/research-and-evidence-survey-2026-07-31.md](docs/research-and-evidence-survey-2026-07-31.md)**

## 요구사항

- Rust 1.70+
- `claude` CLI 설치 및 로그인 (PATH에 없으면 `--claude-bin`)

## 빌드

```bash
cargo build --release   # target/release/research
```

## 서브커맨드

```bash
# 1) 렌즈별 독립 리뷰 + discourse 교차검증(기본 파이프라인)
research --model sonnet --cheap-model haiku review \
  --spec specs/default.toml --document my-research.md \
  --brief brief.md --out runs/pos

# 2) 문서 요약(핵심발견/라벨/분리가능여부) + 확인필요 마커 스캔
research describe --spec specs/default.toml --document my-research.md --out runs/pos

# 3) 개정 제안(추가조사 반영/정정)
research improve --spec specs/default.toml --document my-research.md --out runs/pos

# 4) 자유 질의(ask.md에 누적)
research ask --spec specs/default.toml --document my-research.md --out runs/pos "이 회사가 PCI-DSS 인증을 받았어?"
```

## 실사용 스모크테스트

이 리서치를 낳은 실제 문서(MangroveCafeOrder의 POS 경쟁사 리서치, 510줄)로 `describe`를 돌려 검증했다 —
핵심발견 10개, 15개 섹션 커버리지, "확인필요" 마커 1건을 정확히 뽑아냈다.

## 결정론적 검사 (`checks.rs`)

codereview-loop의 policy.rs+semgrep.rs를 통합했다 — 리서치 도메인엔 "외부 결정론 도구가 자동으로 채워주는 만능 스캐너"(semgrep 대응물)가 없어서 두 모듈로 나눌 이유가 없다는 판단(docs/design-spec.md §3).

| check | 하는 일 |
|---|---|
| citation_density_check | 주장 문장 대비 인용 밀도 |
| source_diversity_check | 인용 중 리서치 대상 기업 자사도메인 비중 |
| numeric_consistency_check | 동일 문구에 서로 다른 수치가 반복되는지(휴리스틱) |
| staleness_flag | 임계값보다 오래된 연도 인용 여부 |
| incentive_disclosure_scan | 리뷰이벤트/협찬 등 인센티브 키워드 |
| access_limitation_disclosure_check | "확인 안 됨"류 정직 표기 존재 여부 |
| dead_link_check | 인용 URL 실제 HTTP 요청(ureq)으로 응답 확인 — `--skip-link-check`로 생략 가능 |

## discourse CHALLENGE 조건 (원본과의 핵심 차이)

codereview-loop은 "근거·반례·범위 등 반박"이면 CHALLENGE로 인정하지만, research-loop은 **"동일 지표를 다른 방법론/다른 소스로 재측정해 수치·주장 불일치를 제기"하는 경우로만 좁힌다**(docs/design-spec.md §4). 근거 없는 취향 반박("오래된 것 같다")은 SURFACE로 강등된다.

## 한계 · 가정

- LLM 점수는 실제 사실검증이 아니다 — 정성 판단 보조용. `citation_status`(VERIFIED/UNVERIFIED/STALE/CONTRADICTED)는 discourse 라운드의 페르소나 판정에 의존하며, CITETRACER류 캐스케이딩 자동검증은 미구현(docs §7).
- `numeric_consistency_check`는 형태소 분석이 아닌 어절 윈도 정규식이라 오탐/누락 가능 — WARN일 뿐 FAIL로 쓰지 않는다.
- 생성 모델과 채점 모델이 같으면 자기 문체를 후하게 본다 — `--cheap-model` 미지정 시에도 경고는 아직 없음(bizplan-loop과 달리 미구현, 추후 보강 여지).
- human-voice 리라이트 단계는 없음(리서치 문서는 톤 재작성이 목적이 아니라는 설계 판단, docs §0).

## 원본

아키텍처 원본: Code-Review-Loop (Loop-Suite/codereview-loop). marketing-loop과 동일한 이식 방법론을 따른다.
