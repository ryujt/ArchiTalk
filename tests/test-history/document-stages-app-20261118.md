# 문서별 설계 생성·체크포인트 재개 App 검증

## 사전 계약과 소유

- 기준 commit `9241110`(0.2.3). 이 담당자는 `src/app.rs`, `src/model.rs`, 공유 합성 CLI fixture, 신규 `tests/design_generation_workflow.rs`와 필요한 기존 App workflow 회귀 hunk 및 이 기록을 소유한다. root는 UI·storage/schema3·통합, 다른 담당자는 Domain의 단계별 prompt/검증 API를 소유한다.
- design logical Run 하나에서 서버 고정순서10문서를 각각 별도 `ai::generate` 요청으로 생성한다. 요청마다 시작 시 캡처한 model/effort/timeout(기본1800·정수10~3600)을 유지한다. 실패 문서에서 중단하고 자동 형식 보정/재시도를 하지 않는다. conversation/critic의 기존 한 번 보정은 유지한다.
- `Session.design_generations`는 serde default Vec으로 과거 generation/provenance를 보존한다. 문서별 validated bytes/hash/source_run/settings/CLI metadata는 기존 snapshot+durable event 트랜잭션으로 commit하며 Session revision만 증가한다. 부분 문서는 Draft/export/readiness가 아니다. 마지막 architecture는 whole validation 전 commit하지 않고 최종 package만 기존 Draft+autoCritic 경로로 확정한다.
- 빈 POST draft는 마지막 미완성 generation의 입력 revision/context hash/contract hash/language/유효 prefix/content hash가 모두 맞으면 재개한다. `restart:true`는 과거 generation을 남기고 새 generation을 만든다. restart는 bool만 허용한다. 성공한 generation은 draft_id로 소비하여 다시 쓰지 않는다. 설정 변경 후 재개는 새 spec을 캡처하되 각 part provenance가 기존 요청과의 차이를 보존한다.
- 호출 전·checkpoint commit·최종 확정에서 현재 Run/세션/checkpoint·revision/hash·cancel/reset/status를 검사한다. 기존 global slot·command receipt/revision·unknown 종료 확인·crash 후 자동 재개 금지를 유지한다. 모델 호출/실제 사용자 프로젝트·DB·서버·공용 설정 조작은0회다.

## 실행 전 계획

| ID | 합성 subprocess 시나리오 | 판정 |
| --- | --- | --- |
| DSG-01 |10단계 정상|canonical 순서10개 durable checkpoint, 같은 Run/spec, 최종 Draft1+critic1만, content/hash/provenance/초기 메시지 보존|
| DSG-02 |중간 문서 invalid → 재개|prefix만 영속화·Draft/critic0, reopen 보존, 새 spec으로 남은 단계만 호출·기존 prefix/source_run 보존|
| DSG-03 |입력 변경/restart/잘못된 restart/hash/prefix|새 generation 분리·이전 보존, strict bool/revision/receipt/slot, 유효성 없는 캐시 미재사용|
| DSG-04 |cancel/reset/stale·재시작 unknown|다음 요청/늦은 checkpoint/최종 Draft 차단, 기존 확인 gate·유저 메시지 불변, 단일 작업 slot 유지|

- 먼저 작은 실제 fake CLI와 동적 JSON snapshot assertions로 baseline을 만들고, Domain API 저장/Model 연결 후 App 단위·통합을 조립한다. fixture는 `TARGET_JSON`에서 기대 종류를 읽고 그 문서 하나만 반환한다. 부모가 쓴 stage-control 파일을 읽어 fail/hold를 주입하며 global counter를 쓰지 않는다. 실제 호출 수는 App progress/Run provenance와 durable 이벤트로 관찰한다.
- 실행 명령과 source 상태/실패/재검증 결과는 각 실행 전에 추가한다. 현재 제품 편집 전 read-only 조사에서 기존 execute는1회 whole-package 생성 후 complete로 넘기며 checkpoint는 없음을 확인했다. 아직 새 회귀 실행 결과는 없다.

## DSG-C01 / DSG-T01 — 조립과 첫 subprocess 회귀

- Domain API가 저장되기 전에는 신규 model이 그 타입을 참조하여 compile 가능한 새 반례가 없었다. 원래 whole-call 동작은 위 read-only 기준선으로 남기고 수정 전 RED를 실행했다고 주장하지 않는다. Model/API 연결 뒤 `cargo check --tests`를 시작했다. 결과는 완료 뒤 기록한다.
- 신규4개 subprocess 회귀를 작성했다. 실행 전 소유 파일만 rustfmt하고 `cargo test --test design_generation_workflow -- --nocapture`를 실행한다. stage fixture의 과거 future-anchor2개는 새 순서 규칙에 맞게 **단계별 응답에만** bare server filename으로 바꾸며 legacy whole response와 원 package fixture는 보존한다.

- DSG-C01: `cargo check --tests` **PASS**, exit0, 6.98초. Domain API/Model/App 연결은 compile을 통과했다.
- DSG-T01: 신규4건 **0 pass / 4 fail**, exit101, compile2.50초+실행10.05초. 첫 단계가 single schema가 아닌 conversation JSON을 반환해3개가 초기에 실패했고 cancel/reset은 prefix를 만들지 못해10초 관측 제한에서 실패했다.
- 분류/수정: **fixture 계약 연결 결함**이다. single prompt의 TASK 첫 줄은 규칙 문서 header이고 실제 `Create exactly ONE...` 문장은 뒤에 있어 startsWith 판별이 맞지 않았다. 공식 `TARGET_JSON`을 파싱한 `target is not None`으로만 copied fake의 기존 design 분기를 선택하도록 수정했다. 제품 검증·횟수·timeout·assertion을 완화하지 않았다. DSG-T02로 같은4건을 재실행한다.

- DSG-T02: **4 pass / 0 fail**, exit0, compile2.45초+실행6.37초. 실제10요청/단일Run·문서별 durable 순서/해시/출처·Draft1/critic1, prefix3 저장/재open/새model·timeout으로 남은7만 재개, bool restart/receipt/fresh input/깨진 hash, 단계 사이 cancel/reset이 모두 통과했다.
- 추가 DSG-T03 계획: 마지막 architecture 거절 때 앞9개만 보존·다음 명시 재개는architecture1+critic1, 진행 중 언어/AI 설정 변경에도10개 captured spec/언어 유지, crash snapshot reopen의unknown 수동 확인/자동 재개0, private pure guard의 revision 없는 source 변화 차단을 추가한다. 기존 App workflow7 및 document_format11 소비자도 함께 실행한다. 명령: `cargo test --test design_generation_workflow --test workflow --test document_format -- --nocapture`, 별도 `cargo test --lib generation_gate_tests`.

- DSG-T03: **25 pass / 0 fail**, exit0. compile4.41초 뒤 새App7건6.79초·기존document_format11건0.71초·workflow7건5.90초였다. 기존 App fixture/legacy 계약 hunk를 약화하거나 바꾸지 않고 shared CLI의 single-target 이식만으로 통과했다. 이 시점 마지막 architecture 실패 주입은 generic invalid Markdown이었다.
- 독립 App/Model 검토는 cancellation/late checkpoint/slot/final Draft에 blocker0으로 보고했다. 진단 한계로 local validation 실패 Run의 CLI metadata는 마지막 성공 stage 값이며 첫 실패면없을 수 있다. 검증되어 저장한 각 part의 provenance는 정확하다. 또한 **local Principal profile은 유효하지만 whole module rule만 실패하는 main**을 App 소비자에서 확인하도록 권고했다.
- DSG-T04 계획: 위 generic 마지막 실패를 main의 Method-R module 본문만 scoped orchestrator/링크가 없는 내용으로 바꾼 copied package로 강화한다. Principal profile 통과를 먼저 assert하고 whole-rule 오류문구·checkpoint9/Draft0/critic0·고친 main만 재개2attempt를 확인한다. 문서 검사/횟수 assertion을 제거하지 않는다. 명령: `cargo test --lib generation_gate_tests`, `cargo test --test design_generation_workflow final_architecture_failure -- --nocapture`. source guard 단위와 강화한 소비자1건만 실행하며 이미 통과한25건 전체는 반복하지 않는다.

- DSG-T04 guard 결과: **1 pass**, exit0, compile2.77초+실행0.01초. revision을 바꾸지 않은 source 변조와 취소된 Launch 모두 실제 단계 시작/commit이 공용으로 쓰는 guard를 통과하지 못했다. 모델/DB 호출은 없다.
- root audit 권고로 design Run/각 part의 effective metadata에 실제 `design_generation::VERSION`을 `prompt_version`/`generation_protocol`로 기록하고 `generation_contract_hash`도 추가했다. conversation/critic metadata는 기존 계약이다. 실행·재사용 판단은 바꾸지 않았다. 전체 정상10개 소비자에 이 metadata 판정을 추가했으므로 DSG-T05는 강화한 마지막main1건과 정상10개1건을 별도로 재실행한다.

- 강화한 마지막 main 소비자 결과: **1 pass**, exit0, compile1.45초+실행3.27초. copied main은 Principal profile을 먼저 통과했지만 정확히 `Method-R module design needs its own orchestrator` whole-rule로 거절됐다. 앞9개 checkpoint/Draft0/critic0을 유지했고 main만 복원한 다음 명시 재개는2attempt(main+critic)로 끝났다. 앞9개 artifact/provenance는 동일했다.
- DSG-T05 정상10개 metadata 소비자: **1 pass**, exit0, compile0.07초+실행3.28초. 각 part의 `prompt_version`/`generation_protocol`은 실제 stages VERSION이고 contract hash가 checkpoint와 같았다. 10개 순서/해시/Run source/spec·최종 Draft/critic·완료 generation 재사용 금지의 기존 판정도 유지했다.
- 최종 `cargo clippy --all-targets -- -D warnings`: **PASS**, exit0, 3.68초. 이후 제품 소스 변경은 없다. 소유 Rust 파일을 rustfmt했고 최종 diff check 결과는 아래에 기록한다. root의 전체 Rust/Node·UI/browser·schema3·설치 결과와 이 표적 결과를 별도로 유지한다.
- 이 담당자는 실제 모델을 호출하거나 사용자 DB/서버/공용 설정을 변경하지 않았다. subprocess는 복사한 합성 CLI이고 fail/hold/hash/crash/reset 주입은 이번 실행 소유 임시 project/SQLite만 대상으로 했다. 첫 T01 4실패와 fixture 판별 수정은 삭제하지 않았다.
- App/Model/fixture 및 신규7개 workflow/guard1개와 이 기록을 동결해 root에게 전달한다. 기존 workflow/document_format 테스트 소스는 수정하지 않았다. root가 schema3/UI 및 최종 전체 검증·commit/push를 소유한다.
- 마감 검사: 소유 Rust3파일 `rustfmt --check --edition 2024`·`git diff --check`·합성 CLI의 Python compile-only 구문 검사가 모두 exit0이었다. 구문 검사는 CLI를 실행하거나 pycache를 쓰지 않았다. 이력까지 최종 동결한다.

## DSG-API01 — 표시용 생성 순서 추가

- root가 실제 mobile PNG에서 일반 CLI progress 문구가 현재 문서명 표시를 덮는 UI 관측 문제를 확인했다. root의 명시 요청으로 workspace 응답에 `document_generation_order`를 `GenerationKind::ALL`의 `{kind,title,filename}` 배열로 계산해 추가한다. 기존 `document_types` 순서·모델·상태기계·입력/계약 hash·timeout은 바꾸지 않는다.
- 이 작은 read-only 응답 추가에는 구현을 복제하는 새 테스트를 만들지 않는다. `cargo check --tests`, 소유 파일 rustfmt/diff check로 조립을 확인하고 root의 새 UI browser 소비자 검증에 연결한다. 실제 모델·사용자 state 조작은 없다.
- DSG-API01 결과: `cargo check --tests` **PASS**, exit0, 2.63초. 소유 `src/app.rs`의 rustfmt check와 `git diff --check`도 exit0이었다. 표시용 배열 추가와 이력만 다시 동결하며 root에게 전달했다.
