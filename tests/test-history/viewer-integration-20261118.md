# 문서 작업공간 통합 검증

## 사전 범위와 계획

- 기준은 `e7bcb0c`(0.3.0)이다. Diagram 담당은 확대·축소·맞춤 표시, UI 담당은 목록형 저장 창·파일 탭·녹음·좌우 패널, backend 담당은 파일 읽기·공용 전사 키·전사 API를 분업한다. root는 시스템 구성도 생성 계약·대표 fixture·README·최종 조립과 배포를 소유한다.
- 실제 사용자 프로젝트·실행 중 서버·공용 키·마이크·클립보드는 건드리지 않는다. 테스트는 임시 폴더와 가짜 CLI/미디어/전사 전송을 사용한다. 실제 OpenAI API와 실제 AI 모델 호출은 이번 검증 대상이 아니다.
- 생성 문서는 10종을 유지하며 주 문서의 `Method-R system design`에 정적 시스템 구성도(Mermaid flowchart)를 추가한다. Method-R 협력 jobflow와 네비게이션·상태·화면 DSL은 유지한다. 새 응답만 검사하며 과거 저장 초안은 변환하지 않는다.

| ID | 사전 판정 기준 |
| --- | --- |
| VI-D01 | 새 주 문서는 실제 system 절의 Mermaid 구성도가 필수. 누락·빈 도표·다른 절/예시/콜아웃 위장은 거절하고 기존 Legacy 단일 문서 계약은 유지 |
| VI-I01 | 파일 경계/secret snapshot/녹음 전 키 확인/리셋 취소/문서 정화와 API wire를 담당자와 교차 검토 |
| VI-T01 | 최종 source의 fmt/clippy/Rust/Node 및 관련 실제 desktop/mobile browser 통과. 실패와 수정은 이력에 보존 |
| VI-R01 | README/API 문서·버전·전역 CLI 설치 확인 후 커밋·푸시. 실제 마이크·유료 전사 미실행 범위 명시 |

## 실행 기록

### VI-D01 사전 반례

새 시스템 구성도를 가진 임시 주 문서와 누락/배치/빈 도표 반례를 먼저 추가하고, 구현 전 `cargo test --locked --test design_package principal_system_structure -- --nocapture`를 실행한다. 기존 fixture는 아직 변경하지 않으므로 새 요구가 없는 현행 validator의 누락 허용을 재현할 예정이다.

- 실제 최초 실행: exit101, `accepted missing/fake system chart:`로 실패했다. 기존 validator가 구성도 없는 새 주 문서를 허용하는 반례를 확인했다.
- 변경: Principal profile에서 실제 system 절의 정적 Mermaid flowchart/graph와 연결선을 확인한다. 예시 fence·주석·콜아웃·다른 절의 도표는 세지 않는다. 생성 prompt에 책임·상태/자료 소유자·경계·화살표 계약을 설명하도록 추가했고 합성 fixture에도 구성도를 넣었다. Mermaid 전체 문법과 설계 의미를 증명하는 검사는 아니다.
- 재검증: `cargo test --locked --test design_package --test document_format --test design_generation_contract` exit0, 42건 통과. Legacy 과거 단일 문서·순차 생성·상호 참조 계약을 유지했다.
- 독립 Critic은 노드 label 안의 화살표 문자열을 실제 edge로 세는 경우와3칸 이내 들여쓴 다른 H2의 범위를 system 절로 오인하는 반례를 임시 compiled probe에서 확인했다. 두 반례를 표적 테스트에 추가한 뒤 다시 실패를 확인하고 lexical edge/기존 ATX heading 인식으로 수정한다.
- 두 반례를 추가한 첫 표적 실행은 label-only 입력 수용으로 exit101이었다. label/quote/comment를 건너뛴 edge 검사와 기존 `atx_heading`의 H1/H2 범위 갱신을 적용한 뒤 위42건이 다시 모두 통과했다.
- root 독립 읽기에서 backend 테스트의 잘못된 DB 파일명·임시 디렉터리 lifetime 및 macOS `/var` alias의 NOFOLLOW 경로 문제를 찾아 담당자에게 전달했다. 담당자는 실제 `workspace.db` 검사, tempdir 유지와 trusted config ancestor canonicalization 후 dirfd 고정으로 수정했다. 사용자 키/DB 접근은 없었다.
- 기존 hostile 문서 browser 검사는 새 시스템 Mermaid1개를 포함하도록 관찰 대상을 바꾸고, 정화 후 renderer가 생성한 확대 버튼만 예외로 허용한다. 원문 HTML에는 이 예외 class가 허용되지 않으며 `data-action`/on*/원격 요청/실행 probe/토큰 markup 금지 검사는 유지한다. 실제 통합 browser 실행은 모든 소스 동결 뒤 수행한다.

### VI-T01 backend 동결 뒤 전체 Rust 검증 계획

- backend 소스는 담당자의27+5 표적 통과 후 동결했다. 버전을0.4.0으로 올리고 `cargo check --offline`으로 lock을 갱신한다. 이 변경은 DB schema3과 기존1800/3600초 제한을 바꾸지 않는다.
- 첫 `cargo fmt --check`는 root 소유의 구성도 validator/test 줄바꿈 차이로 exit1이었다. `cargo fmt`를 적용한다. 다음 fmt check와 `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`로 모든 Rust 계약을 검증한다. UI 정적 자산은 개발 중이며 브라우저 검증용 바이너리는 UI 동결 후 별도로 빌드·복사한다.
- 첫 전체 clippy는 `items_after_test_module`로 exit101이었다. 새 전사 오류 변환 helper가 app의 기존 cfg(test) module 뒤에 있었으므로 동작 변경 없이 module 앞에 옮겼다. 전체 Rust test는 별도 프로세스에서 실행 중이며 결과를 아래에 기록한다.
- 결과: `cargo check --offline`·수정 후 `cargo clippy --all-targets -- -D warnings` exit0. 전체 `cargo test --locked`는 **180 passed / 0 failed / 12 ignored**(별도 child probe의 중복 summary 제외)였다. ignored는 실제 CLI/model·구 바이너리 opt-in 사례다.
- 마지막 lexical 검사에는 unquoted `User's workspace`처럼 label 안의 apostrophe가 따옴표 시작으로 오인되지 않도록 한 분기와 수용 사례를 더했다. 이 변경 뒤 `cargo fmt`와 `cargo test --locked --test design_package principal_system_structure`를 별도로 실행한다. 최종 clippy/fmt는 UI 동결 뒤 확인한다.

### VI-V01 실제 브라우저 음성 소비자 사전 계획

Frontend 담당과 분담해 root는 `tests/browser/document-voice.spec.mjs`를 소유한다. fresh binary+임시 프로젝트와 실제 DOM으로 다음을 검증한다: 키 누락 오류가 문서 탭에서 보이며 마이크 호출0, 가짜 녹음 중 중복 제목의 정확 anchor/선택 문장 수집, audio-only multipart1회와 전사 지시문 자동 복사, 클립보드 거부 시 수동 복사 창, pagehide 중 마이크 정리·추가 POST0. MediaRecorder/getUserMedia/clipboard와 전사 HTTP 응답은 모두 stub이다. 실제 하드웨어·공용 비밀 파일·클립보드·OpenAI·모델 사용은0이다. UI 변경은 담당자에게 되돌리고 이 테스트에서는 제품 소스를 수정하지 않는다.

- 첫 바이너리 `target/architalk-viewer-voice-initial` SHA256 `d1ecc1ee230a22eb2fed58d5f711a19868df93f9459cd363f19962f1583df7e8`로 `ARCHITALK_BROWSER_BINARY=target/architalk-viewer-voice-initial ARCHITALK_BROWSER_RUN=viewer-voice-initial-20261118 npx playwright test tests/browser/document-voice.spec.mjs`를 실행했다.
- 첫 desktop의 키 누락 메시지는 DOM에 생겼지만 visibility 검사가 실패했다. 실제 PNG를 보니 왼쪽 sidebar를 display:none으로 접자 CSS auto-placement가 main을 너비0인 첫 grid column에 옮기고 aux가 가운데 열을 차지했다. 문구·문서가 세로로 잘리는 실제 회귀였으며 frontend 담당에게 명시 grid column/areas와 양쪽 패널 geometry 검증을 요청했다. 결과를 숨기기 위해 테스트 selector를 완화하지 않는다.
- 첫 실행 최종은 **4 pass / 2 fail**(37.7초)로 desktop/mobile의 같은 너비0 오류만 실패했다. 정상 전사·선택 anchor·audio-only·클립보드 fallback·pagehide cleanup은 통과했다. 수정 후에는 음성 설정 form의 키 write-only·빈칸 유지·명시 삭제를 추가한8사례를 검증한다. 이 form 테스트의 모든 키와 API도 synthetic route stub이다.

### VI-T02 통합 UI 재검증 계획과 기준 바이너리

- 명시 grid column, file tab race guard, 기록 위치·plain text line, pagehide cleanup을 담은 `target/architalk-viewer-fixed`는0.4.0이며 SHA256 `427323fe65e4806639e2fce9a1105b0e238958717fe926ac623766bbd24a4a71`이다. 각 담당자의 browser 실행도 이 파일을 사용하고 이후 소스 수정 시 필요한 범위를 다시 검사한다.
- `npm run test:web`: **60 pass / 0 fail / 1 skip**(629ms). skip은 외부 mdview 소스를 지정하는 opt-in 호환 사례다. `cargo fmt --check`·최종 `cargo clippy --all-targets -- -D warnings` 모두 exit0.
- root는 이 고정 binary로 voice8을 재실행하고 기존 workspace/settings/chat-scroll/reset/retry/document-stages/timeout/design-package/document-rendering/diagram-zoom desktop/mobile 사례를 통합 실행한다. opt-in generated-package는 별도로 선택한 합성 fixture로 실행 가능하며 실제 모델을 호출하지 않는다. 신규 tabs/panels/save 사례는 frontend 담당, delayed renderer race4사례는 독립 reviewer가 맡는다.
- fixed voice 결과: **8/8 pass**, exit0,7.4초(`viewer-voice-fixed-20261118`). 키 없는 실제 visible 오류/마이크0, write-only 설정 form 유지·삭제, 중복 제목 `같은-제목-1`와 선택 문장, 음성만 들어간 multipart1회, 자동 복사·거부 fallback·pagehide track 정리가 모두 통과했다.
- 독립 reviewer의 file race는 같은 binary로 **4/4 pass**, exit0,4.9초였다. 자세한 초기 실패·검증자의 RO timing 보정과 최종 geometry는 viewer-diagram 이력에 남긴다.
- 최종 호환 확인은 사용자 제공 mdview 소스를 읽기 전용 import하는 `ARCHITALK_MDVIEW_ROOT=/Users/ryu/projects/personal/tools.camp/cli/mdview node --test tests/smartmd-compat.test.mjs`를 실행한다. 외부 소스 수정/실행 서버/모델 요청은 없다.
- mdview 호환은 **6/6 pass**, exit0,135ms였다. fixed voice의 desktop 문서/모바일 수동 복사 PNG를 root가 직접 열어 중앙 폭·녹음 제어·선택 본문·수동 결과와 하단 버튼의 가시성을 확인했다.
- `cargo build --release --locked` exit0(14.84초). UI 브라우저가 실행되는 동안 배포 바이너리만 미리 빌드했으며 전역 설치는 나머지 UI 확인 뒤 수행한다.
- 기존 대화·설정·저장·초기화·시간 제한·문서 단계·정화·도표 browser 통합은 **30/30 pass**, exit0,3.2분(`viewer-regression-20261118`)이었다. 이30개 안에 diagram8개가 포함되므로 별도 diagram 표적과 중복 합산하지 않는다.
- frontend의 새 workspace browser는 **4/4 pass**, exit0,14.2초(`viewer-workspace-b05-20261118`)였다. canonical `/var` alias 및 natural export 경로의 초기 검증자 오류와 수정은 viewer-workspace 이력에 구분한다. 총 서로 다른 최종 browser 사례는 **46/46 pass**(기존/diagram30+voice8+race4+workspace4)다. 같은 고정 바이너리를 사용했고 이후 제품 소스 변경은 없었다. root도360px mobile 저장 PNG에서 전체 저장 버튼과 닫힌 문서 목록을 직접 확인했다.

### VI-R01 배포 확인 계획

README/API 문서에 의도·10종 결과물·Method-R 구성도·프로젝트 파일 보기·패널·녹음/전사/복사와 저장 경계를 반영했다. 제품0.4.0을 `./install.sh`로 기본 전역 경로에 설치하고 `/tmp`의 새 login shell에서 PATH와 `architalk --version`을 확인한다. 검증/이력 변경만 최종 확인한 뒤 커밋·푸시한다. 실제 사용자 서버는 종료하지 않고 재시작 안내만 전달한다. 실제 마이크/클립보드/OpenAI 전사·실제 AI 모델 호출은 이번 실행에서 하지 않았다.

- `./install.sh` exit0, 기존0.3.0을0.4.0으로 교체했다. `/tmp`에서 각각 새 login shell로 확인한 `command -v architalk`는 `/Users/ryu/.cargo/bin/architalk`, `architalk --version`은 `architalk 0.4.0`이었다. 실제 프로젝트 App/DB를 열거나 서버를 종료하지 않았다.
- 최종 `git diff --check` exit0. 제품/테스트/README/API/검증 이력만 포함하며 target의 binary·PNG·오디오/키는 커밋하지 않는다. 커밋·push의 실제 결과는 Git 기록과 최종 응답으로 확인한다.
