# 문서 종류별 생성·진행 저장·재개 — 2026-11-18

## 승인 범위와 계약

- 사용자 승인: 종류별 요청으로 요청당 생성량을 줄이고 성공한 문서를 보존해 실패 지점부터 이어간다. 기본1800초/최대3600초는 유지한다. 완료 후 README 갱신·전역 설치·커밋/푸시를 수행한다.
- 요청 순서는 서버 고정이다: drivers → quality_scenarios → alternatives → decisions → delivery → jobflow → state → screen_layout → navigation → architecture. 추가 계획용 모델 호출은 없다. 각 요청은 strict `{kind,markdown}` 하나만 반환한다.
- 같은 logical design run의 모델/생각 수준/시간 제한은 캡처한 값을 유지하고, 문서 요청마다 동일 제한을 적용한다. 재개는 새 run이므로 현재 신뢰된 설정을 새로 캡처하며 각 문서에 source run/설정/CLI provenance를 보존한다. 모델 자동 전환이나 형식 오류 자동 재호출은 없다.
- Session의 별도 design_generations 기록에 검증된 문서와 hash를 DB에 원자 저장한다. 원 대화·설계 입력 버전·완성 Draft는 중간 생성에 영향받지 않는다. 진행 기록은 생성 입력/언어/계약 hash와 prefix가 일치할 때만 재사용한다. 완료 generation은 재사용하지 않으며 명시 restart는 새 generation으로 구분한다.
- 미래 trace ID는 소유 문서 생성 전까지 정의 의무로 추적하고 그 차례에 반드시 해소한다. 미래 파일에는 bare link만 쓰며 실제 생성된 문서의 anchor만 참조한다. 마지막 주 문서를 포함한 기존 전체 패키지 검증을 통과해야 Draft가 된다. 전체 비판 검토와10파일 저장은 유지한다.
- 중간 문서는 승인·저장 가능한 Draft가 아니다. 마지막 전체 검증이 실패하면 마지막 문서를 완료로 기록하지 않으며, 재개와 별도로 처음부터 새 생성도 제공한다. 취소·reset·입력 변경·종료 미확인은 기존 차단 정책을 유지하며 자동 재개하지 않는다.
- 구 schema2 바이너리가 새 진행 기록을 무시하고 잃어버리지 않도록 schema3로 올린다. 기존 v1/v2는 WAL 확정 내용을 포함한 사전 backup 후 marker를 갱신하며 원 세션/receipt/event는 보존한다.
- 역할/소유: Domain Developer(단일 문서 prompt/검증), App Developer(Model/오케스트레이션/fixture/E2E), 독립 Critic(계약·반례·최종 diff), Root(UI/저장 schema/호환 검증/문서/설치/조립). 기존 상속 모델을 유지한다. 사용자의 실제 DB·공용 설정·현재 서버는 수정하지 않는다.

## DS-01 사전 검증 계획

- 새 Domain 계약의 strict single response·기존 SmartMD/Method-R/전용 DSL·trace/anchor prefix 및 전체 검증을 먼저 확인하고 App에 조립한다. 단위 실패/수정은 [Domain 기록](document-stages-domain-20261118.md)에 남긴다.
- 실제 제한 subprocess fixture로10요청→전체 Draft/critic, 중간 실패→reopen/명시 재개→앞 문서 재호출 없음, 입력/계약 변경·restart·취소/reset·최종 검증 실패를 확인한다. [App 기록](document-stages-app-20261118.md)에 최초 실패와 결과를 남긴다. 실제 모델을 호출하지 않는다.
- Root는 먼저0.2.3 schema2 release 바이너리를 test artifact에 복사해 보존한다. schema3 migration과 v2 WAL backup/receipt/event/진행 기록 보존, 구 실물 바이너리의 forward guard를 임시 프로젝트에서 확인한다.
- UI는 진행 중 완료 수·현재 문서·요청별 제한을 표시하고 실패 뒤 남은 문서 재개/처음부터 생성 선택을 제공한다. 기존 초안과 입력·문서 선택, 메시지 중복 없음·자동 재실행 없음·부분 문서 저장 금지를 Node와 실제 isolated browser에서 검증한다.
- 동결 후 fmt/clippy·전체 Rust/Node·관련 browser를 실행한다. 새 schema/사용법/총시간과 요청 횟수의 한계/기존 문서 호환을 README/구현 안내에 반영하고0.3.0 전역 설치와 commit/push를 완료한다.

## DS-UI-01 Node 사전 계획

- 완료3/10의 실패 generation을 recovery/error에서 표시하고 완료된 문서의 원문은 렌더/승인/저장하지 않는 반례를 먼저 추가한다. 명시 재개는 기존 `/draft` 빈 payload, 처음부터 생성은 `{restart:true}`이며 둘 다 busy/종료 미확인/reset 차단을 유지한다.
- Node 대역은 응답을 기다리는 동안 생성 count/현재 run.progress/요청당1800초를 표시하는지 확인하고, 입력/언어가 달라진 오래된 checkpoint 및 이미 draft가 된 generation을 재개 대상으로 표시하지 않는지 검사한다. 메시지·기존 초안·문서 선택·미전송 입력 보존과 명령0/1 경계를 확인한다.
- Root의 새 storage 테스트는 사전 계획 뒤 작성했다. Domain/App가 모델 API를 조립 중이므로 해당 API가 컴파일 가능한 시점에 migration/기존 storage 회귀·구 바이너리 forward guard를 실행한다. 이전0.2.3 실물은 `target/legacy-binaries/schema2-0.2.3/architalk`에 보존했고 버전 출력을 확인했다. 사용자의 DB는 열지 않았다.

### DS-UI-01 결과와 후속 계획

- 수정 전 Node3개: 1 PASS / 2 FAIL(exit1, 약718ms). 진행 수/재개 선택이 없고 restart 명령이 실행되지 않는 반례를 확인했다.
- UI 진행 표시와 명시적 restart 연결 뒤 같은3개: 3/3 PASS(exit0, 약190ms). 중간 Markdown은 표시/승인/저장하지 않으며 현재 문서·완료 수·1800초 제한을 보여준다. 입력·언어가 달라진 기록과 완료 generation은 재개 대상으로 표시하지 않고 unknown 종료/busy 차단을 유지한다.
- 다음은 전체 Node 두 suite, schema3/기존 storage 표적을 실행한다. App 오케스트레이션은 담당자가 조립 중이므로 컴파일 경합은 제품 동작 실패와 구분하고 기록한다. 표적이 컴파일되면 보존한 구 실물 바이너리의 forward guard도 별도 opt-in으로 실행한다.

### DS-UI-02 독립 검토 지적과 반례 계획

- Critic은 cached 설정 언어가 최신 workspace 언어를 가리는 경우 잘못된 재개 안내를 표시하고, 이전 완성 초안을 비판 검토하는 동안 최신 미완성 generation의3/10을 표시하는 반례를 발견했다. 실제 대상·최신 snapshot을 기준으로 선택하게 수정한다.
- 두 Node 반례를 먼저 실행하고 수정 후 같은 두 건 및 전체 Node를 확인한다. 모델 요청을 바꾸는 지적이 아니라 실제 표시 대상과 명시 재개의 일치 문제다.

### DS-UI-02 결과

- 최초2개는0 PASS / 2 FAIL이었으며 critic 반례의 첫 대기가 active run에도 recovery를 기대하는 테스트 observer 결함이었다. active run ID 관찰로 고친 뒤 두 건 모두 실제 표시 문제로 FAIL(exit1, 약156ms)했다. 최초 결과를 통과로 세지 않았다.
- 최신 workspace 설정을 우선하고 설정 GET/PATCH 결과도 최신 revision일 때 workspace에 반영한다. 비판 검토의 count는 실제 최신 검토 Draft ID에 연결된 완료 generation에서 가져온다. 같은2개2/2 PASS, 전체 Node43/43 PASS(exit0, 약577ms). 모델 원문·중간 Markdown을 실행하거나 게시하지 않는다.

## DS-STORAGE-01 결과

- `cargo test --test schema3_migration --test storage_contract -- --nocapture`: 14 PASS / 1 opt-in ignored(exit0). 새 v2 WAL backup과 기존 v1/receipt/event·inode·symlink·FIFO·락 종료 회귀가 모두 통과했다.
- `ARCHITALK_SCHEMA2_BINARY=<보존한0.2.3> cargo test --test schema3_migration the_actual_schema2_binary -- --ignored --nocapture`: 1/1 PASS(exit0, 0.05초). 구 실물은 schema3를 `unsupported_schema`로 거절하고 DB 바이트를 바꾸지 않았다. 모델 호출은 없다.

## DS-BROWSER-01 사전 계획

- 임시 프로젝트/복사한 fake CLI/부모가 atomic하게 쓰는 stage-control.json만 사용한다. 실제 CLI 권한/SQLite/HTTP/SSE/UI를 통과하고 실제 모델은 호출하지 않는다.
- desktop/mobile에서3문서완료→decisions 형식실패→부분 저장/승인 없음→reload해도3/10유지/자동run0→실패 해제 후사용자 resume→완료3개동일 hash/source_run 유지+나머지7개새run→전체Draft/critic/10파일저장을 확인한다.
- 같은 case에서 다시 생성한 prefix 실패 뒤 명시 restart를 누르면 새 generation이 만들어지고 기존 문서/완성 Draft를 그대로 보존하는지도 확인한다. 중간 화면·가로 overflow·pageerror·외부 요청·사용자 메시지 중복0을 판정한다.
- source freeze 뒤 새0.3.0 binary를 run 소유 복사본에 고정하고 신규2 viewport와 기존 문서표시/실제timeout·설정·reset 관련 표적을 위험에 맞춰 실행한다. 실패는 도구/fixture/제품 원인을 구분해 남기고 근거 없이 기준을 낮추지 않는다.

## DS-FINAL-01 검증 계획

- Domain/App 생산 소스가 동결됐다. App의 마지막 주 문서는 개별 Principal profile을 통과하지만 전체 module-link 검증만 실패하는 경우9개 보존/Draft·critic0→고친main1요청+critic1 소비자 반례까지 통과했다. Root는 최종 `cargo fmt --check`, `cargo test --locked`, Node43, `git diff --check`를 확인하고 담당자의 all-target clippy 결과를 조립한다.
- 이후 `cargo build --bin architalk`로 갱신하고 run 소유 고정 binary를 복사해 신규 문서별 생성 browser2 viewport를 먼저 실행한다. 통과 뒤 관련 기존 browser 회귀를 실행하며 실제 모델은 사용하지 않는다.

### DS-FINAL-01 정적·회귀 결과와 browser 연결

- `cargo test --locked`: 168 PASS / 12 ignored / 0 FAIL(exit0). 유료 모델/추가 설치 진단/구 바이너리 opt-in은 기본 실행에서 제외했고 schema2 구 실물은 위에서 별도 통과했다.
- `cargo fmt --check`, `git diff --check`: PASS. App 담당자의 같은 source 최종 `cargo clippy --all-targets -- -D warnings` PASS(3.68초). Root Node는 지적 반영 후43/43 PASS다.
- fresh `cargo build --bin architalk` PASS(5.15초), 고정 binary0.3.0의 SHA256은 `29a080b1443096e24dff1fe614836672a6246239eff73a57963ebf4cb0014213`이다. 신규2 viewport를 `document-stages-new-20261118` artifact run으로 시작했다.
- 기존 package browser의 두 future-file 링크 기대를 새 계약에 맞춰 이식한다. Screen Layout→Navigation, Jobflow→마지막 Architecture는 정확한 bare filename을 검증하고 문서 전환을 확인한다. 이미 생성한 Layout으로 가는 Navigation의 실제 anchor focus, 같은 파일/NFD/중복 heading focus와 경로·스크립트 차단 검사는 그대로 유지한다. 제품 품질 규칙을 낮추는 변경이 아니다.

### DS-BROWSER-01 신규 결과와 회귀 계획

- 신규 staged-generation desktop/mobile2건 모두 PASS(exit0, 24.6초, retries0). cp3 보존/reload/새 run0→기존3개 동일 hash·source run+남은7개 새 run→Draft/critic/명시 새 generation/10파일 저장·user message1까지 통과했다. source0.3.0과 위 SHA를 고정했다.
- 같은 고정 binary로 기존 workspace/package/chat-scroll/document-rendering/project-reset/settings/timeout-recovery/conversation-retry8파일의20 case를 실행한다. 공유fixture·기본 generation entry가 모두 바뀌었으므로 해당 소비자를 확인하며 신규2개는 불필요하게 반복하지 않는다. 요청당1800/상한3600 계약과 실제10초 timeout/cancel·단계별 partial 저장 금지를 구분한다.

### DS-BROWSER-02 초기 회귀·시각 확인에서 발견한 보완

- 신규 desktop 실패 보존 화면과 mobile 재개 화면 PNG를 실제 열었다. native CLI progress가 저장된 run.progress를 덮으면서 문서 종류 대신 'AI CLI를 시작합니다'만 표시되는 결함을 관찰했다. 서버의 고정 순서를 `document_generation_order`로 제공하고 UI가 실제 다음 종류를 표시하도록 보완한다. timeout·생성 순서·state machine은 바꾸지 않는다.
- 첫 기존20-case 실행은10 PASS / 2 FAIL / 1 interrupted / 7 not run(exit130, 1.9분)으로 보존했다. package의 추가 assertion이 renderer의 안전 href(`#`+WeakMap navigation)를 실제 filename으로 착각한 테스트 observer 결함이었다. 저장된 Markdown의 bare filename과 실제 문서 이동을 함께 검사하도록 고친다. settings의 마지막 모달 열기에서 page closed가 발생한 원인은 아직 미확정이다. 실패와 source 보완을 확인한 뒤 이 run 소유 Playwright 프로세스만 SIGINT로 중단했다. 사용자 브라우저·서버는 조작하지 않았다.
- 다음 검증은 현재 종류 Node 반례/전체 Node, fresh0.3.0 고정 binary로 신규+package+settings의 desktop 표적부터 시작한다. settings 실패가 반복되면 lifecycle/duration 증거로 원인을 분류하며 조건을 근거 없이 완화하지 않는다. 이후 같은 binary의 나머지 표적을 확인한다.

### DS-BROWSER-03 보완 후 desktop 표적

- 현재 문서 표시의 Node 반례와 전체 suite44/44 PASS(exit0, 약603ms). 서버 고정 순서로 현재 종류를 선택해 native CLI progress가 바뀌어도 표시를 유지한다. App의 변경은 조회 응답의 생성 순서 metadata 추가뿐이며 stage state machine은 그대로다.
- 새 고정0.3.0 binary SHA256: `1aa40aa49a38435089d5987dd0bacb015121cef2fe098854f1c4db082b912e1c`. `document-stages-focus-20261118`에서 desktop package/stages/settings3건 모두 PASS(exit0, 33.24초). 각각7.64/10.20/14.54초였고 retries0·flaky0였다. report는 `target/browser-reports/document-stages-focus.json`에 보존했다.
- settings page-closed 원인은 재현되지 않아 미확정으로 남긴다. 생성 동작·제한·설정 assertion·테스트 제한60초를 바꾸지 않은 동일 소비자가14.54초에 완료했다. 오류를 무시하거나 성공으로 바꾸지 않았으며 최초 실패 artifact는 그대로다.
- 다음은 같은 최종 binary의 mobile11건(신규+기존전체)과 desktop 나머지8건을 순차 실행한다. 이미 최종 source로 통과한 desktop3건은 불필요하게 반복하지 않는다. 최종22개 고유 case의 결과를 함께 기록한다.

### DS-BROWSER-04 mobile 결과

- 최종 SHA binary의 mobile11/11 PASS(exit0, runner표시1.5분, retries0). 신규 staged resume·전체파일저장과 기존 스크롤/답변재시도/문서연결/안전한renderer/설정/실제10초timeout/기록삭제·복구를 모두 통과했다.
- 수정된 mobile 재개 PNG를 실제 열어3/10·현재 아키텍처 결정 기록·요청당1800초·기존대화/입력창의 배치를 확인했다. native CLI progress가 현재 종류를 가리지 않는다. settings도같은assertion/기존60초case제한으로완료해이전page-closed가재현되지않았다.

### DS-BROWSER-05 최종22개 판정

- 동일 최종 SHA binary로 desktop 나머지8/8 PASS(exit0, 57.6초, retries0). desktop focus3 + mobile11 + desktop remainder8 = 고유22/22 case PASS다. 이전10-pass/2-fail/interrupted run과 최초 신규2건은 최종 source 결과와 구분해 보존했다.
- Critic은 표시 보완의 최신 설정 revision/검토 Draft 연결/현재 문서와 CLI 진행 분리 및 조회 metadata의 hash/state-machine 비영향을 읽기 전용 재검토했고 blocker 없음으로 사인오프했다. App의 마지막 whole-rule-only 실패 소비자 반례도 검토했다.
- 마지막 조회 metadata 추가 후 최종 source의 fmt·전체 Rust·all-target clippy·diff를 한 번 더 확인한다. Node는 같은 최종 UI의44/44 결과를 사용한다. 이후0.3.0 전역 설치/프로젝트 밖버전 확인/커밋·푸시를 수행한다. 실제 모델 호출과 사용자 DB·서버·공용설정 변경은 없다.

## DS-FINAL-02 최종 소스 재검증·설치 계획

- 최종 `cargo test --locked` PASS(exit0, 168 PASS/12 ignored/0 FAIL). 재실행 로그는 `target/document-stages-final-rust.log`에 보존했다. `cargo fmt --check`, `git diff --check` PASS, 최종 `cargo clippy --all-targets -- -D warnings` PASS(3.41초).
- 최종 UI의 Node44, 동일 최종 source의 browser22 고유 case PASS 및 schema2 구 실물의 forward guard1 PASS를 조립했다. 부분 문서가 완성 Draft로 노출되지 않고 기존 결과/기록·기본1800/최대3600·계정/공용 설정을 보존한다. 실제 모델의 생성 속도/응답 품질을 이번 synthetic fixture 결과로 보장하지 않는다.
- `./install.sh`로0.3.0 release를 전역 교체하고 프로젝트 밖 `/tmp` login shell에서 버전을 확인한다. 사용자 프로젝트의 첫 재실행 시에만 schema3 migration/backup이 수행된다. Root는 현재 사용자 서버/DB를 중지·삭제·이행하지 않는다.

## DS-RELEASE-01 설치 결과

- `./install.sh` PASS(exit0), release build7.56초. 전역 실행파일을0.2.3→0.3.0으로 교체했다.
- 프로젝트 밖 `/tmp` 새 login shell에서 `architalk --version` → `architalk 0.3.0`(exit0). 기존 사용자 서버·DB·공용 설정은 그대로 유지했다.
- README/구현 안내/검증 색인에 문서별 요청·재개/명시 restart·전체 검토/저장·schema3 사전 backup·기본1800/최대3600 유지·총시간/사용량 한계를 반영했다. 사용자는 기존 서버를 종료하고 같은 프로젝트에서 새 버전을 실행해 적용한다.
