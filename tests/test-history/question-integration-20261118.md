# 대화 질문 검증·명시적 복구·favicon — 2026-11-18

- 사용자 오류: 첫 conversation의 `invalid_model_output: question exceeds field limit`와 `/favicon.ico`404. 현재 프로젝트의 SQLite는 metadata만 read-only 확인했다. Claude2.1.327 terminal 응답 후 field 검증에서 실패했고, user message1/assistant0/question0 상태였다. 기존 raw model 응답은 보존되어 있지 않아 어느 field를 초과했는지는 확정할 수 없다.
- source 근거: question은 target200/text2000/reason8192/option1000 UTF-8 bytes, options4개 상한 및 NUL 금지이나 prompt에는 숫자 계약이 없고 오류가 원인을 구분하지 않는다. 기존 대화/critic의 보정1회는 유지하고 추가 자동 재시도를 넣지 않는다.
- 소유: Domain Developer(domain/단위), Critic→좁은 retry API/별도계약, Tester 실제 fixture-subprocess 보정/persistence, root favicon/UI조립/실제합성대화/전체검증/설치/README/커밋푸시. 이력은 question-contract/question-workflow/conversation-retry로 분리한다.
- 사용자 DB·현재 실행 서버를 삭제/수정/자동 재시도하지 않는다. 복구는 사용자가 누르는 retry로 새 run만 만들고 이미 저장한 user 메시지를 복제하지 않는다.

## QINT-01 계획

- favicon은 포함된 로컬 ICO를 `/favicon.ico`에서 적절한 image MIME으로 제공하고 HTML에서 지정한다. HTTP200·바이너리icon signature·CSP/동일origin경계를 확인한다. 제품 실행에 CDN/이미지 빌드가 필요하지 않다.
- domain 수치/prompt 단일계약·정확한 field/count/bytes/limit 오류·NUL·atomic 거절, App의 최대1회 보정 및2회실패 이력, 명시적 conversation retry의 revision/idle/receipt/이전 내용 보존을 검증한다.
- source 동결 후 fmt/clippy·Rust·Node 및 retry/favicon 소비자 browser 표적을 실행한다. 실제 Claude의 합성 첫 대화 작업을 기본 모델/생각 수준으로1회만 별도 실행한다(기존 보정최대1회 포함, 실제 프로젝트 자료전송 없음). generic `{ok:true}` smoke와 달리 실제 Domain prompt→App validation 경로를 확인하며 모델 사용량이 발생한다.

## QINT-01 결과와 최종 검증 계획

- `cargo test --test favicon`: HTTP 1/1 PASS. API 인증 없이 icon200·ICO signature·MIME·nosniff·HTML link와 foreign Host403 확인.
- `cargo test --test real_conversation -- --ignored --nocapture`: 실제 Claude2.1.327, 모델/생각 수준 CLI 기본값의 합성 첫 대화1/1 PASS(14.16초). App의 실제 prompt→CLI→schema/domain 검증→영속화 경로로 user1/assistant1·표시 질문1·선택지2개를 확인했다. 사용자 프로젝트 자료는 읽거나 전달하지 않았다. 이 실행 이후 실제 모델 재호출은 계획하지 않는다.
- 최종 검증은 0.2.2 기준 `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, Node 두 suite를 실행한다. browser는 Tester의 retry desktop/mobile 및 기존 reset 표적 결과로 확인한다.
- 통과 후 `./install.sh`로 전역 갱신하고 프로젝트 밖 새 login shell에서 버전을 확인한다. 현재 사용자 서버·프로젝트 DB는 그대로 유지하며 사용자가 재시작 후 기존 대화의 retry를 선택한다.

## QINT-02 최종 정적·회귀 결과

- 0.2.2 `cargo test --locked`: 141 PASS / 11 ignored. 실제 모델·설치 CLI 진단·구버전 바이너리 전용 항목은 기본 실행에서 제외한다. 별도의 실제 첫 대화1건 결과는 위 QINT-01에 기록했다.
- `node --test tests/web.test.mjs tests/design-package-ui.test.mjs`: 35/35 PASS. Node의 기존 module type 추론 안내만 있으며 테스트 실패는 없다.
- `cargo fmt --check`, `git diff --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS(3.40초).
- Critic의 비작성 부분(domain contract·retry UI·favicon) read-only 검토에서 blocker/중요 회귀 없음. retry API는 Root가 변경 diff·회귀 계약을 별도로 확인했다. 사용자 데이터 조작과 추가 실제 모델 호출은 없었다.

## QINT-03 전역 설치

- `./install.sh`: release build7.22초, 전역 실행파일을 0.2.1→0.2.2로 교체(exit0).
- 프로젝트 밖 `/tmp`의 새 login shell에서 `architalk --version`: `architalk 0.2.2`(exit0). 제품 서버를 실행하거나 기존 사용자 서버를 중지하지 않았다.
- 브라우저 표적의 초기 실패·진단·재검증은 [question-workflow](question-workflow-20261118.md)에 모두 보존한다. 최종 판정은 해당 검증 완료 뒤 아래에 추가한다.

## QINT-04 브라우저 최종 판정

- 필수 4건 PASS: B06의 명시적 retry desktop/mobile2건(5.2초)과 B02의 기존 project-reset desktop/mobile2건. 제품 소스를 고정한 binary는 version bump 직전 0.2.1이며 SHA와 정확한 실행은 동료 이력에 기록했다. 이후 제품 변경은 0.2.2 버전 표기뿐이다. 0.2.2 소스의 Rust/Node/clippy 및 실제 전역 설치 결과와 구분한다.
- retry는 첫 논리 작업의 형식 보정2attempt 실패 뒤 reload→사용자 클릭1회→새 attempt1회의 성공을 확인했다. 이전 failed run과 원 사용자 메시지를 그대로 유지하며 최종 user1/assistant1·failed1/completed1·create/retry POST2였다.
- exact `/favicon.ico`는 Node HTTP200·`image/x-icon`·6449bytes이고, 브라우저 query probe의 SHA가 동일하며 Chrome64×64 decode까지 PASS했다. 요청에 API 인증을 추가하지 않았다. query는 요청 가로채기 중 exact favicon을 강제 abort하고 관련 이벤트도 숨기는 설치된 Playwright1.65.1의 특수 처리를 피하기 위한 테스트에만 사용한다. 제품의 icon URL은 `/favicon.ico`다.
- 초기 B01~B05 실패는 도구의 favicon 차단과 보정 prompt를 읽지 못한 synthetic fixture의 JSON 파서 결함으로 분류하고 원본을 보존했다. 테스트 보정 뒤 제품/보안/UI 코드를 추가 수정하지 않았다. 사용자 프로젝트·DB·현재 서버와 공용 AI 설정은 변경하지 않았다.
