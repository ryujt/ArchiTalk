# 질문 필드 한도와 한 번의 보정 워크플로 검증

## 사전 범위

- 기준 commit: `7273e60`. domain 담당자는 `src/domain.rs`와 domain 계약 테스트를, root는 HTTP/favicon과 조립을 소유한다. 이 담당자는 새 `tests/question_workflow.rs`와 이 기록만 수정한다.
- 현재 질문 제한은 UTF-8 bytes 기준 `target_ref` 200, `text` 2000, `reason` 8192, 선택지 0~4개, 각 선택지 1000이며 모든 문자열에서 NUL을 거절한다. 원래 오류는 모두 `question exceeds field limit`이고 생성 prompt에는 수치 계약이 없다. App은 conversation/critic 출력 검증 실패 때 correction prompt를 한 번만 보내며 design은 재생성하지 않는다.
- 이전 workflow 기록의 subprocess/SQLite 회복과 domain의 atomic clone 계약을 참고했다. 이번에는 동일 오케스트레이션에서 질문의 정확한 오류·원출력 전달·수정 성공·수정 실패 후 기존 내용 보존만 검사한다. 실제 모델·사용자 DB·사용자 브라우저·공용 설정을 호출/변경하지 않는다.
- 테스트는 macOS의 실제 제한 subprocess와 임시 project SQLite를 사용한다. fake Claude executable은 검증된 CLI event 형식만 출력하고, 첫 응답과 보정 응답을 prompt 구조로 구분한다. 공유 counter 파일은 쓰지 않는다. App의 generation progress event 수로 호출 횟수를 독립 관찰한다.

| ID | 입력/시나리오 | 필수 판정 |
| --- | --- | --- |
| QW-01 | 선택지 5개 → 정상 두 번째 응답 | 첫 오류에 `questions[0].options`·관측5·상한4, correction prompt의 원 prompt/오류/원 JSON 일치, 실제 subprocess 정확히2회, 정상 질문과 assistant 응답만 저장 |
| QW-02 | 질문 본문 `가`1000개+`?`: 1001문자/3001bytes → 정상 두 번째 응답 | 문자수와 byte수 구분, `questions[0].text`·관측3001·상한2000 오류, 동일 보정 구조, 정확히2회, 최종 completed 및 DB reopen 보존 |
| QW-03 | 첫 선택지 초과 → 두 번째 UTF-8 본문 초과 | 세 번째 시도 없음, `invalid_model_output`과 두 번째 precise 오류, 실패 출력의 claim/coverage/reply/question 미반영, 이미 commit된 사용자 입력·기존 claim/질문/초안 보존, reopen 뒤 동일 상태 |

## 실행 기록

### QW-B01 — 수정 전 기준선 계획

- 새 subprocess 회귀 3건을 먼저 작성하고 `cargo test --test question_workflow -- --nocapture`를 실행한다. 기존 generic 오류 때문에 보정 prompt 검사 또는 최종 precise 오류 검사가 실패할 것으로 예상한다. 실제 결과는 실행 뒤 별도 기록하며 예상으로 통과를 선언하지 않는다.
- baseline source가 다른 담당자의 구현과 경합할 수 있으므로 실행 직전 domain/app SHA를 기록한다. domain 구현 freeze 후 관련 표적만 다시 실행한다. root가 full Rust/fmt/clippy를 소유하며 별도 불필요한 전체 회귀는 반복하지 않는다.

- QW-B01 결과: **0 pass / 3 fail**, exit101, 실행1.02초, compile0.54초였다. 실행 직전 domain SHA-256은 `d98de21351f1338796c7ce8d08cbe189a89315aefc2eb770d9117ac06792f74d`, App은 `fbfe9846dd196628820c390b4f0797bc452406770f33798e9541f194867ecb08`이었다. source에 기존 `question exceeds field limit`가 남아 있는 것을 실행 뒤에도 확인했다.
- 세 fake CLI 모두 보정 prompt에서 precise field 오류 검사를 통과하지 못해 종료했고 App은 `cli_failed`로 종료했다. 따라서 정상 두 번째 응답의 저장/atomic 최종 판정 전, verified init event가1개뿐인 지점에서 실패했다. 이1개는 **성공 초기화 event 수**이며 subprocess 생성 횟수가1회였다는 뜻은 아니다. precise error를 받은 경우에만 fake가 정상 응답하므로, 숫자/경로 정보가 없는 기존 오류로는 통과할 수 없는 회귀다.
- 실행 후 새 테스트 파일만 `rustfmt --edition 2024 tests/question_workflow.rs`로 정리했다(exit0). root의 병렬 App retry·HTTP 작업은 수정하지 않았다. source freeze 뒤 QW-R01로 동일 세 건을 재검증한다.

### QU-N01 — 명시적 답변 재시도 UI 사전 계획

- root가 UI 소유를 `web/app.js`, `tests/web.test.mjs`, 신규 `tests/browser/conversation-retry.spec.mjs`와 isolated fixture helper로 확대했다. API는 POST `/api/v1/sessions/{id}/conversation/retry`에 기존 빈 payload envelope를 보내며202 `{session,run}`을 반환한다. 서버의 최근 conversation 실패/중지/중단과 confirmed/manually_confirmed 종료만 허용한다.
- 먼저 Node 반례로 recovery·error 양쪽의 primary `답변 다시 받기`, 빈 payload/revision/UUID·중복 클릭 억제·동일 사용자 메시지·inline pending, unknown 종료/다른 작업/성공 작업의 재시도 금지를 확인한다. 그 뒤 UI만 최소 연결한다. 수동 종료 확인 전 gate와 기존 대화 복귀/초안 생성 선택을 유지한다.
- 브라우저는 임시 fake CLI에 읽기 전용 control 입력을 추가하여 첫 logical conversation의 두 attempt 모두 선택지5개로 거절한 뒤 parent가 정상 응답으로 전환한다. reload recovery에서 명시 재시도하고 새 logical run 하나/실제 attempt 하나, 입력 중복 없음·기존 failed run 보존·favicon200/MIME·project reset gate를 확인한다. 사용자 탭/DB·실제 provider 호출은 없다.

- QU-N01 수정 전 결과: `node --test --test-name-pattern='failed conversation offers explicit retry|conversation retry respects' tests/web.test.mjs`, **0 pass / 2 fail**, exit1, 약141ms. recovery에 답변 재시도가 없고 eligible conversation이 재시도 버튼을 제공하지 않는 반례를 확인했다.
- UI 변경: 서버와 같은 recent-run 자격으로 error/recovery의 primary `답변 다시 받기`를 표시하고 빈 envelope를 새 retry API에 보낸다. 기존 `command`의 busy·연결 gate와 `adoptResult`의 epoch/inline pending을 재사용한다. 원 사용자 메시지·작성중인 입력은 추가/삭제하지 않고 다른 종류/성공/종료미확인 작업에서는 요청 자체를 막는다.

### QW-R01 / QU-N02 — domain freeze 후 관련 검증 계획

- domain 담당자가 precise 오류·한도 prompt·기존 bound 수치 보존을 동결했다. `cargo test --test question_workflow -- --nocapture`와 `node --check web/app.js`, `node --test tests/web.test.mjs`를 실행한다. 서로 다른 임시 자원을 사용하는 독립 계층이며 root의 전체 Rust 검사와 별도 결과로 기록한다.

- QW-R01 결과: **3 pass / 0 fail**, exit0, compile1.62초+실행0.90초. options5와 1001문자/3001bytes 질문 모두 precise 오류를 correction prompt에서 받았고 두 번째 정상 응답을 저장했다. 두 응답이 모두 잘못된 경우 실제 verified init2회로 끝나고 최종 `invalid_model_output`·두 번째 본문 오류·원 내용 및 reopen 동일성을 통과했다.
- QU-N02 결과: `node --check web/app.js` exit0, Node 전체 **29 pass / 0 fail**, exit0, 약473ms. 새2개 회귀 외 기존 연결/epoch/수동종료/설정/문서/저장/삭제 경계를 함께 유지했다.

### QU-B01 — 실제 브라우저 retry/favicon/reset 표적 계획

- 신규 retry spec은 desktop/mobile 각각 첫 논리 conversation의 두 subprocess 응답을 options5개로 거절시켜 precise 오류와 failed run1개를 관찰한다. parent만 제어하는 임시 JSON은 overflow/hold boolean뿐이고 child는 읽기만 한다. SSE 연결이 준비되면 parent가 hold를 해제하므로 arbitrary sleep 없이 App의 verified init progress2개를 직접 관찰한다.
- 실제 HTML reload 뒤 recovery primary 버튼이 남는 것을 확인한다. parent가 다음 응답을 정상으로 바꾼 뒤 명시 retry API를 한 번만 호출한다. 동일 SSE 관찰로 attempt1개, run2개(failed/completed), 사용자 메시지1개/assistant1개, 빈 payload와 expected revision, inline pending, 가로 넘침0·pageerror/외부 요청0을 판정한다. 실제 favicon 자동 요청200·`image/x-icon`·ICO signature와 capability header 부재도 검사한다.
- `node --check`로 신규 spec/helper를 확인하고 fresh build를 run 소유 binary에 고정한다. 명령은 `ARCHITALK_BROWSER_BINARY=<고정 binary> ARCHITALK_BROWSER_RUN=question-retry-targeted-20261118 npx playwright test tests/browser/conversation-retry.spec.mjs tests/browser/project-reset.spec.mjs`다. 신규 retry2+기존 실제 삭제/reset2의4건만 실행하며 root의 전체 Rust 검증과 별도 결과다.

- QU-B01 준비: 신규 spec/helper 구문과 `git diff --check` exit0. `cargo build --bin architalk` exit0, 5.72초 뒤 고정한 run 복사본은 `target/browser-binaries/question-retry-targeted-20261118/architalk`, SHA-256 `881f20ad6c72939eed6ff7cd1ef3195460a9d20ad0cbd1dd2200ab43216d4173`, 33,593,144bytes다. root의 병렬 build/설치는 이 binary를 바꾸지 않는다.
- QU-B01 결과: desktop 신규 case가 첫 welcome에서 favicon 자동 요청을 기다리다가60초 timeout으로 **1 fail**했다. 이번 headless Chrome은 선언된 browser-chrome favicon을 자동 요청하지 않았고, 실제 retry 생성 단계에 도달하지 않았다. helper control은 여전히 hold=true였으며 임시 runtime run 디렉터리도0개라 모델/fixture generation이 없었다. 제품404가 관찰된 결과가 아니라 **테스트의 headless favicon 로딩 가정 결함**이다.
- 첫 timeout 뒤 불필요한 나머지 실행을 중지했다(exit130). 이미 시작된 desktop reset1개는 interrupted/ECONNREFUSED, mobile2개는 not run이며 통과로 집계하지 않는다. 실패 PNG와 timeout 원본은 해당 artifact run에 보존했다.
- 수정: 실제 페이지의 선언된 `link[rel=icon]` 경로를 먼저 검증하고 같은 browser page의 fetch로 그 asset을 명시 요청한다. 실제 서버200/MIME/ICO signature/Authorization header 부재 기준을 유지하며 제품 CSP/route를 바꾸지 않는다. QU-B02에서 같은 고정 binary로4건을 다시 실행한다.

- QU-B02 결과: **reset2 pass / retry2 fail**, exit1, 8.8초. reset 삭제·SSE·기록 비재생성·연결 gate는 두 viewport 모두 통과했다. 신규 retry는 여전히 생성 전 favicon의 browser fetch가 `TypeError: Failed to fetch`여서 중단됐다. B01의 headless 자동 요청 가정만으로 원인을 설명할 수 없으므로 **미확정**으로 재분류한다. route oneshot과 같은 origin의 앱 자산/API가 정상이라는 증거를 바탕으로 favicon requestfailed/CSP 진단을 보강한다.
- QU-B03 계획: 동일 고정 binary의 신규 case desktop1개만 실행하여 resource 요청 실패와 CSP effectiveDirective/blockedURI를 확인한다. 불필요한 pending favicon promise는 없애고 실제 fetch의 status/MIME/바이너리 signature를 직접 반환받는다. 제품 예외 무시나200 판정 완화는 하지 않는다.

- QU-B03 결과: **0 pass / 1 fail**, exit1, 첫 favicon fetch에서 중단했다. requestfailed와 CSP violation이 모두0이었으며 선언 경로는 `/favicon.ico`였다. 원인이 네트워크 계층에서 아직 관찰되지 않아 제품 문제라고 단정하지 않는다. QU-B04에서 같은 case desktop1개에 exact resolved href/base/origin·asset response/console과 server origin의 Node fetch 결과를 함께 기록한다. root도 같은 비교를 제안했다. 실제 retry/모델 생성 단계는 여전히 실행 전이다.

- QU-B04 결과: **0 pass / 1 fail**, exit1. exact `/favicon.ico` Node fetch는200·`image/x-icon`·6449bytes였다. 브라우저 resolved href/base/origin도 모두 실제 같은 서버였고 CSP/console/requestfailed는0이며 favicon response 자체가 관찰되지 않았다.
- 확정 원인: 설치된 Playwright1.65.1의 `node_modules/playwright-core/lib/coreBundle.js`14555가 `url.endsWith('/favicon.ico')`를 `_isFavicon`으로 표시하고24942~24946 `requestStarted`에서 routing이 있는 favicon을 `route2?.abort('aborted')`로 자체 중단한 뒤 이벤트를 보내지 않는다. response/requestfailed 역시 specialcase로 숨긴다. 따라서 B01~B04는 제품404/CSP나 일반 headless 동작이 아니라 **테스트 프레임워크 abort**였다. 원래 실패와 조사 순서는 보존한다.
- QU-B05 계획: 선언된 link href `/favicon.ico`와 exact path Node200/MIME를 유지한다. browser fetch/decode에만 쿼리 `?browser-probe=1`를 붙여 같은 HTTP path/handler를 Playwright 특수 처리 없이 검사하고 Node exact bytes SHA-256과 일치를 요구한다. browser의 실제 ICO 디코딩·양수 natural dimensions·Authorization 부재·외부 route guard를 모두 유지한다. 제품 소스/CSP를 바꾸거나 favicon 검증을 건너뛰지 않는다. 같은 고정 binary에서 신규 retry2개만 재검증하며 B02에서 이미 통과한 reset2를 또 반복하지 않는다.

- root 확인: 고정 browser binary 버전은 **0.2.1**이며 이번 제품 변경 소스를 모두 포함한 version bump 직전 artifact다. root의 최종0.2.2 Rust/Node/clippy/install 결과는 별도 통합 기록이다. version 문자열만 달라 관련 browser를 재빌드/반복하지 않는다.
- QU-B05 결과: 신규 **2 fail**, exit1. 두 viewport 모두 exact Node icon200/MIME 및 실제 browser fetch/decode·같은 bytes SHA를 통과해 이전 favicon oracle 문제는 해소됐다. 이후 오류 UI는 expected `invalid_model_output` 대신 `cli_failed`였고, 회귀가 원하는 두 번째 invalid candidate까지 도달하지 못했다.
- 원인: 기존 일반 fake workflow는 `json.loads(prompt.split('CONTEXT_JSON:\\n',1)[1])`로 JSON 뒤 전체를 읽는다. 이번에 처음 보정 suffix가 붙은 prompt를 받으면 JSON extra-data 오류가 나는 **fixture 계약 결함**이다. App 전용 QW3건의 보정 구조/atomic 동작은 이미 통과했으며 제품 retry 횟수/오류/파서 변경은 하지 않았다.
- 수정: 새 overflow helper를 활성화한 **이번 테스트의 copied CLI**에서만 `JSONDecoder.raw_decode(...)[0]`로 원 CONTEXT_JSON 첫 객체를 읽게 한다. child는 여전히 read-only이며 공유 fixture 원본·다른 browser case 기본 동작은 그대로다. 구문 검사 exit0. QU-B06은 동일 고정 binary로 신규2개만 재실행한다.

- QU-B06 결과: 신규 retry desktop/mobile **2 pass / 0 fail**, exit0, **5.2초**, retries0. B02에서 동일 product binary로 통과한 실제 reset2건과 합쳐 필수4건을 충족한다. 제품 소스·CSP·timeout 기준을 바꾼 재실행이 아니라 B01~B05에서 확인된 framework/fixture 결함만 보정했다.
- 최종 두 viewport의 독립 SSE/API 관측값은 모두 `initialAttempts=2`, `retryAttempts=1`, `failedRuns=1`, `completedRuns=1`, `userMessages=1`, `assistantMessages=1`, `postCommands=2`였다. 서버202 retry payload는 빈 객체이며 실패 직후 session revision을 사용한다. 기존 failed run은 byte-for-byte JSON equality로 유지하고 두 번째 run만 completed로 저장됐다. 실제 HTML reload recovery·primary button·inline pending·동일 사용자 입력 보존·가로 넘침/pageerror/외부 요청0을 통과했다.
- favicon은 exact path의 Node200·`image/x-icon`·6449bytes와 browser query의 SHA-256이 일치했고, 실제 Chrome 이미지 디코딩이 **64×64**였다. asset 요청의 Authorization은 없었다. 쿼리는 Playwright의 suffix specialcase만 피하며 Axum의 동일 path/handler, 제품 CSP, 외부 요청 guard는 그대로다. 독립 reviewer도 Playwright 공식 `frames.ts`와 interception 회귀에서 같은 의도적 abort 계약을 확인해 이 보정에 동의했다.
- 실제 시각 확인: 최종 desktop 오류 PNG를 열어 precise field error와 primary `답변 다시 받기`, 대화/초안 대안을 확인했다. mobile inline pending PNG도 실제 열어 기존 사용자 입력 하나 뒤에 답변 작성 상태와 composer가 함께 있고 가로 넘침이 없는 것을 확인했다. 최종 run에는8 PNG와 두 `retry-boundary.json`이 `target/browser-artifacts/question-retry-targeted-r4-20261118` 아래 남는다. 원래 실패/중단/진단 artifact run도 보존한다.
- 독립 UI 검토: reviewer가 자격 predicate·snapshot의 latest-run 선택·global active/unknown/busy/reset gate·빈 payload·`command/adoptResult` epoch 재사용을 읽고 blocker0으로 보고했다. UI가 사용자 메시지나 draft를 직접 추가하지 않는 것을 재확인했다.
- 마감 확인: `node --check web/app.js`, 신규 helper/spec 구문과 `git diff --check`가 exit0이었다. 제품/UI 변경은 QU-N02의 Node29통과 이후 없으며, App subprocess3건·Node29건·관련 browser4건이 최종 통과했다. root의0.2.2 전체 Rust/Node/clippy/설치·실제 합성 대화1건 결과는 통합 기록에서 별도 관리한다. 이 담당자는 실제 모델을 호출하지 않았고 실제 사용자 DB/탭/공용 설정을 조작하지 않았다.
- 소유 파일을 동결하여 root에게 전달한다: `web/app.js`, `tests/web.test.mjs`, `tests/question_workflow.rs`, `tests/browser/fixtures.mjs`, `tests/browser/conversation-retry.spec.mjs`, 이 기록. commit/push와 설치는 root 소유다.
