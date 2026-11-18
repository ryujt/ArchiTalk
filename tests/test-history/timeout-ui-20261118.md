# 설계 생성 시간 제한과 복구 UI 검증

## 사전 범위와 읽기 전용 조사

- 기준 commit: `59ce1a4`(0.2.2). 사용자 증상은 설계 생성 중 `AI CLI reached its wall-time limit.`다. 실제 사용자 DB·서버·브라우저·공용 설정을 조작하거나 모델을 호출하지 않는다. 현재는 source 계약 확정 전이므로 제품/테스트 코드를 편집하지 않는다. 이 기록만 이 담당자가 소유한다.
- 근거: `web/app.js`의 `pendingMessage/generating/errorStage/recovery/settingsModal/openSettings`, `tests/web.test.mjs`, `tests/browser/settings.spec.mjs`, 이전 [question 통합](question-integration-20261118.md)·[package 통합](design-package-integration-20261118.md)·[package browser](design-package-browser-20261118.md)을 읽었다.
- 현재 설정 기본은300초, UI/서버 수치 계약은 정수10~600초다. 이 수치를 늘리거나 migration하는 판단은 root가 소유하며 아직 새 계약은 동결되지 않았다. 이미 시작한 Run의 `effective_settings.timeout_seconds`는 고정되고 공용 설정 변경은 다음 작업부터 적용된다.
- 기존 package의 보존 실제 응답 한 건은251.84초가 걸렸다. 이는 timeout의 가능한 경계 사례이며 이번 사용자 요청의 실제 적용 제한/경과/정체 이유를 확인한 결과는 아니다. 사용자 상태를 읽어서 원인을 추정하지 않는다.
- 확인한 UI 반례: pending은 작업 종류만 알리고 적용된 초를 표시하지 않는다. 오류 화면은 영문 wall-time 원문과 일반 설정 버튼, 의미가 모호한 `현재 답변으로 다시 정리`를 제공한다. 재접속 recovery에는 timeout 이유/적용 제한이 없어 설정을 바꾸거나 설계 생성을 다시 요청하는 선택을 찾기 어렵다.

## 최소 개선 제안과 동결 대기

- `run.error.code === 'timeout'`을 기준으로 한국어 설명을 제공하고 해당 Run에 고정된 적용 제한을 표시한다. 현재 설정 값으로 과거 Run 제한을 다시 해석하지 않는다. pending에서도 적용 제한과 다음 작업 적용 범위를 짧게 안내한다.
- 전용 `시간 제한 조정`은 기존 settings dialog를 열고 timeout input에 focus한다. 변경의 공용 범위/다음 작업 적용과 자동 재실행 없음은 유지한다. 모달 취소/저장 동안 현재 scene·선택 문서·입력·기존 초안 내용을 보존한다.
- 설계 실패의 명시 재시도는 `초안 다시 생성`처럼 대화의 `답변 다시 받기`와 구분하고 기존 POST `/draft`를 재사용한다. 사용자 메시지를 추가하지 않으며 이전 failed run/기존 완성 초안을 보존한다. 종료미확인 상태는 기존 수동 recover gate를 우선한다. 검토 실패를 설계 재생성과 혼동하지 않는다.
- 상한/기본값·과거 설정 보존 정책·정확한 문구/버튼 우선순위는 root의 동결 계약을 기다린다. 새 policy를 추측하여 source를 바꾸지 않는다.

## 합의 후 최소 검증 계획

| ID | 계층/입력 | 필수 관측 |
| --- | --- | --- |
| TUI-N01 | Node, timeout인 design Run과 현재 다른 설정값 | 적용된 Run 제한·한국어 복구 설명·전용 settings action·명시 design retry, conversation/critic/unknown 종료 구분 |
| TUI-N02 | Node, settings 열기/취소/적용과 retry | timeout field focus·scene/입력/선택/기존 draft 유지, 저장만으로 새 run0, 명시 `/draft`만1POST·빈 payload/현재 revision·user 메시지 중복0·inline pending |
| TUI-B01 | 실제 isolated browser desktop/mobile, fake CLI의 design 응답만 지연 | 실제 wall-time timeout과 confirmed 종료→설정 조정→명시 retry 성공, 과거 Run 제한 불변·새 Run 변경값·기존 draft 보존·사용자 메시지 중복0·pageerror/외부 요청/가로 넘침0 |

- 새 contract 확정 뒤 먼저 Node 반례와 실행 전 기록을 추가한다. 기존 timeout bounds/진행중 Run 고정/다음 Run 적용 회귀는 계약 변경이 있는 항목만 이식한다. root가 Rust/config/AI/통합 검사를 소유하고 이 담당자는 UI 소비자와 관련 browser 표적을 소유할 예정이다.
- browser는 이번 실행 소유 temp project/config/copied fake CLI만 사용하며 실제 모델 호출을 할 수 없다. 필요하다면 parent가 제어하는 read-only fixture 입력으로 **design만** 지연시켜 실제 최소 제한을 통과하게 하고 다음 retry를 정상 응답으로 바꾼다. 공유 counter/사용자 provider home은 쓰지 않는다. 기본 app timeout과 테스트 전용 주입을 구분해 기록한다.
- source 동결 후 fresh build를 run 소유 binary에 복사해 SHA/버전을 기록한다. 실제 브라우저/SQLite/subprocess 경계를 통과한 결과와 Node 대역 결과를 구분하며 기존 실패·미실행 상태를 삭제하지 않는다. 구체적인 실행 명령/결과는 실행 전에 다음 절에 추가한다.

## 실행 기록

- 현재 테스트 실행 없음. 읽기 전용 조사와 위 계획만 작성했으며 새 timeout contract의 동결 통지를 기다린다.

### TUI-N01 — 확정 계약과 수정 전 반례 계획

- root가 단일 timeout 기본1800초(30분), 정수10~3600초, 역할별 새 필드 없음, 명시적300 등 기존값 유지/다음 작업 적용 계약을 동결했다. backend config/AI는 별도 담당자 소유다. UI는 field bounds/help/default fallback, Run의 실제 적용 제한 표시, timeout code의 한국어 복구, timeout field focus, design의 `초안 다시 생성`을 소유한다.
- root와 browser 주입 방식을 합의했다. 제품 min10을 바꾸지 않으므로 실제 App의1초 deadline을 browser에 주입하지 않는다. browser에서 설정10초+copied CLI의 design만30초 지연→실제 timeout/confirmed 종료→1800초 설정→정상 재생성을 desktop/mobile 각1회 검증한다(합계 약20초 대기). 기존 adapter의 직접 RunSpec1초 종료 회귀와 브라우저 실제10초를 구분한다. 모델 호출/제품 test hook은 없다.
- Node에서 새 경계1800/3600 허용·3601 거절, timeout recovery/error의 실제 적용300초, settings focus/open/close의 scene/입력/선택/초안 보존, 명시 draft POST1회와 pending의 새 적용값, critic/conversation/unknown 종료 구분·적용 제한 누락 시 임의값 미표시를 먼저 반례로 만든다. 그 뒤 관련 전체 Node 회귀만 실행한다.

- 수정 전 실행: `node --test --test-name-pattern='request time limits|timeout recovery' tests/web.test.mjs`, **0 pass / 3 fail**, exit1, 약157ms. 기존600 상한이 새 유효값을 거절하고 recovery에 적용된300초/재생성 안내가 없으며 critic timeout에서 완성 초안 선택이 primary가 아닌 반례였다. 최초 실패를 그대로 보존한다.
- 최소 UI 수정 뒤 동일 집중3개는 **3 pass**, exit0, 약173ms였다. 새 timeout default 누락 fallback1800 회귀를 추가하고 root의 요청별 제한 구분을 반영한 최종 `node --test tests/web.test.mjs tests/design-package-ui.test.mjs`는 **38 pass / 0 fail**, exit0, 약523ms였다. 알려진 module type 추론 안내 외 실패는 없다. `node --check web/app.js`도 exit0이다.
- 문구는 **요청당** 제한이다. conversation/critic의 기존 형식 보정은 새 요청에 같은 제한을 적용하고 별도 비판 검토도 새 요청이므로 논리 Run 전체가 표시된 초 안에 반드시 끝난다고 안내하지 않는다. 설정 help에 이를 설명하고 pending에는 `요청당 시간 제한`과 다음 작업 적용을 표시한다.
- `timeout` code의 한국어 설명·접힌 원본 오류·적용 제한·전용 settings input focus를 추가했다. design 재생성은 기존 `/draft` 빈 envelope이고 대화 입력/기존 draft/선택 문서를 건드리지 않는다. 설정 모달만 열거나 취소/저장하면 자동 재실행하지 않는다. error/recovery에서 기존 draft를 다시 생성할 때는 review pending에 기존 선택 문서를 보여준다. critic 실패는 완성 초안 보기를 우선하며 unknown 종료는 수동 확인 전 generate를 막는다.
- 독립 reviewer는 실제 Run snapshot만 표시하는 정책·미확인 종료/global gate·전용 GET/open/focus·명시 POST/draft·user duplication 없음·선택/입력/초안 보존을 읽고 blocker0으로 보고했다. 코드 편집이나 별도 모델 실행은 하지 않았다.

### TUI-B01 — 실제 timeout 및 설정 브라우저 표적 계획

- root의 backend/UI 계약이 동결됐고 최종 product 버전은0.2.3이다. 신규 `tests/browser/timeout-recovery.spec.mjs`2 viewport와 기존 `tests/browser/settings.spec.mjs`2 viewport를 fresh build의 고정 binary로 실행한다. 기존 전체 browser suite를 이유 없이 반복하지 않는다.
- 신규 흐름: 실제 fixture conversation→10문서 draft/critic 완료→state 문서 선택→설정10초→**design만**30초 지연한 실제 제한 subprocess→timeout/confirmed 종료와 기존 draft 보존→전용 settings input focus/취소/1800 저장(자동 run0)→명시 재생성→기존 state preview/pending/new1800 적용→새 draft/critic 성공. 사용자 메시지1/이전 failed run/기존 draft bytes는 유지하고 요청·자원·overflow를 판정한다.
- additive `configureDesignDelay`는 부모만 쓰는 임시 JSON을 atomic replace하고 copied CLI의 design 분기만 그 파일을 읽는다. 첫 지연은 실제 App10초 deadline으로 중단되고, retry는 parent hold를 해제해야 정상 fixture 문서를 반환한다. 실제 모델/사용자 설정/사용자 DB/프로젝트 test hook은 없다. 다른 case는 이 helper를 호출하지 않으므로 기본 동작이 바뀌지 않는다.
- 기존 settings spec에서는 min10/max3600·3601 거절로 계약만 이식했고 유효480 저장/reload·기존 Run 불변·진행중240 유지/다음120 적용 기준을 유지했다. helper/spec/settings 구문과 `git diff --check`는 exit0이었다.
- 실행 명령: `cargo build --bin architalk`, 고정 복사본 생성 뒤 `ARCHITALK_BROWSER_BINARY=<run 소유 binary> ARCHITALK_BROWSER_RUN=timeout-recovery-final-20261118 npx playwright test tests/browser/timeout-recovery.spec.mjs tests/browser/settings.spec.mjs`. SHA/버전/실제 결과는 실행 뒤 아래에 추가한다.

- build 결과: exit0, **6.37초**. `target/browser-binaries/timeout-recovery-final-20261118/architalk`로 복사한 run 소유 binary는 `architalk 0.2.3`, **33,568,472bytes**, SHA-256 **1b02ec729add1b5c7e18a4fd5ea1ea7725bde8e5700104eeb00bcc56eda10eaf**였다. 다른 cargo 실행이 이 복사본을 바꾸지 않는다.
- TUI-B01 결과: **4 pass / 0 fail**, exit0, **52.9초**, retries0. 신규 실제 timeout desktop/mobile2개와 기존 settings desktop/mobile2개를 같은 제품 artifact로 통과했다. 이번 browser 실행의 실패/재시도는 없으며 Node 수정 전3실패는 위 N01에 보존한다.
- 두 신규 case의 실제 관측: `actualTimeoutSeconds=10`, `errorCode=timeout`, `termination=confirmed`, `changedLimit=1800`, `retryLimit=1800`, `userMessages=1`, `drafts=2`, `preservedOldDraft=true`, `postCommands=4`였다. POST는 최초 conversation create1회와 초기/timeout/명시 retry draft3회뿐이다. timeout 때 자동 design 보정·재실행이나 사용자 메시지 복제가 없었다. 설정 취소와 저장만으로는 Run 목록이 바뀌지 않았고, 실패한 Run의10초와 현재/다음 Run의1800초를 분리해 표시했다.
- 기존 settings2건도 max3600/3601 거절을 통과했으며 숫자480 PATCH/저장/reload, 기존 Run 불변, 진행중240 고정과 다음120 적용을 유지했다. 새로운 기본값1800이 명시된 기존 설정을 덮어쓴다고 판정하지 않는다. 이는 root의 config/App migration·old explicit값 검증과 별도 UI 소비자 결과다.
- 실제 시각 확인: desktop `02-timeout-recovery-actions.png`를 열어 한국어 timeout·적용10초·접힌 세부 오류·primary 시간 제한 조정·초안 재생성/이전 초안 대안을 확인했다. mobile `03-focused-time-limit.png`를 열어 해당 numeric input의 focus ring·입력1800·10~3600 안내·요청별 보정/검토와 다음 작업 적용 설명, modal 줄바꿈을 확인했다. 스크린샷/상태는 합성 테스트 프로젝트만 포함한다.
- artifact: `target/browser-artifacts/timeout-recovery-final-20261118`에18 PNG와 두 `timeout-boundary.json`을 보존한다. 실제 사용자 DB/서버/탭·공용 설정을 변경하지 않았고 실제 모델 호출은0회다. 실제 deadline은 제품 min10을 준수했으며1초 browser timeout으로 허위 보고하지 않는다.
- 최종 `node --check`(app/helper/settings spec/new spec)와 `git diff --check`는 exit0이었다. source/test/history를 동결하고 root에게 전달한다. 소유 파일은 `web/app.js`, `tests/web.test.mjs`, `tests/browser/settings.spec.mjs`, `tests/browser/fixtures.mjs`, `tests/browser/timeout-recovery.spec.mjs`, 이 기록이다. root가 별도의 최종 Rust/Node/clippy·설치·버전·commit/push를 관리한다.
