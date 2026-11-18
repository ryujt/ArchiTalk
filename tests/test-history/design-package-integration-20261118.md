# Method-R 종류별 설계 패키지 통합

- 기준: `d35a386` renderer 수정 커밋 위의 새 문서 패키지 작업. 사용자는 강의의 드라이버→품질속성→택틱/스타일/패턴→트레이드오프→결정→ADR 흐름을 종류별 문서로 요구했고, C4 대신 Method-R 및 Navigation/Jobflow/State/Layout 유지, 완료 시 README의 의도·결과물 설명을 요청했다.
- 계약: 주 아키텍처 + 서버가 정한 9종 문서, 공통 SmartMD·종류별 내용·Method-R·DRV/QA/ALT/ADR/VAL 연결, 전체 패키지 critic, aggregate source hash와 10개 파일의 개별 hash/manifest-last 발행. 기존 단일 초안/발행 복구를 지원한다.
- 참고: [도메인 계약](design-package-domain-20261118.md), [저장·발행 계약](design-package-storage-20261118.md), [UI 문서 선택·링크](design-package-ui-20261118.md). renderer의 과거 12건 통과는 새로운 다중 파일 계약의 통과로 대체하지 않는다.
- 사용자 프로젝트의 저장 문서와 상태 DB는 분석만 했고 수정하지 않는다. 새 동작은 소스와 격리된 합성 프로젝트에서 검증하며, 실제 모델 probe에는 합성 자료 조회 도구만 전달한다.

## PKG-INT-01 — 조립과 회귀

- 상태: 첫 전체 Rust 101건 통과(모델/구버전 binary 5건 ignored), Node 27건 통과. fixture Python AST·JSON/JS 구문, rustfmt 통과. 패키지와 schema2를 구분할 수 있도록 제품 버전을 0.2.0으로 갱신했다. 이후 독립 검토의 HTML block 반례를 수정했으므로 최종 조립 검사는 해당 변경을 포함해 다시 수행한다.
- 방법: Python fixture AST/JSON 파싱, `node --check web/app.js`, 기존 및 추가 웹 계약, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`, 실제 browser 전체 회귀.
- 기대: 정확히 10종 생성/종류 선택/허용 문서·anchor 이동/전체 문서 저장/새로고침 복구, legacy 단일 파일 호환, 세부 문서만 바뀐 preview 차단, WAL 포함 백업과 schema2 forward guard. 모델 호출 없는 테스트와 실제 probe 결과를 구분한다.

## PKG-REAL-01 — 실제 Codex 패키지 1회

- 상태: 기본 전체 회귀 및 HTML block 위장 반례 수정의 집중 39건 통과 후 실제 호출 시작. 모델이 정확한 문법을 추측하지 않도록 새 패키지 규칙에 SmartMD·4종 DSL의 문법 예시도 포함했다. 단일 문서용 과거 프롬프트의 서로 충돌하는 산출물 규칙은 포함하지 않는다.
- 명령: `cargo test --test real_document_format -- --ignored --nocapture`.
- 격리된 합성 목록/상세 웹 도구로 design `ai::generate`를 한 번 호출하고 10종·Method-R·trace/link/SmartMD 구조를 검사한다. 자동 repair나 재시도는 하지 않는다. 이 probe는 300초 제한을 명시하고 모델 사용량이 발생한다. 기본 Rust suite에서는 ignored다.
- 성공/실패 응답과 생성 문서는 Git 제외 `target/document-probes/<uuid>/`에 보존한다. 실제 사용자 문서·인증 정보·접속 토큰은 전달하거나 출력하지 않는다. 실제 엔진 출력 확인과 문서 내용의 사실적 정확성을 구분한다.

## PKG-INSTALL-01 — README·전역 실행

- 상태: 필수 검증 후 README를 실제 의도/설계 과정/10종 결과물/사용·검토·저장 방식으로 갱신하고 `./install.sh`로 전역 실행 파일에 반영한다. 사용자 실행 중 서버는 종료하지 않는다.

## 검토와 문서 정합성

- README와 implementation의 모든 로컬 Markdown 링크 존재를 검사했다. UI 담당의 읽기 전용 검토로 10종 파일명·종류 선택·전체 저장·Method-R·설정·설치 경로가 구현과 일치함을 확인했다. 자동 보정 호출(대화/critic 1회, design 없음), 첫 schema2 이행 시점의 백업, 과거 초안 선택, 소스 설치 의존을 정확히 설명하도록 보완했다.
- root는 storage/app/files diff의 고정 파일명, source aggregate hash와 파일별 hash의 구분, 백업/VFS 경계와 legacy 복구를 검토했다. UI cache·문서 callback·로컬 anchor는 서버 카탈로그와 실제 draft만 사용한다.
- Critic이 `<pre>`에 숨긴 필수 절/ID를 실제 문서로 세는 반례를 발견했다. 해당 agent가 재현 실패 후 수정한 변경은 독립 검토와 자체 구현을 구분하며 [도메인 기록](design-package-domain-20261118.md)에 보존한다. root는 수정 diff와 최종 suite로 다시 검증한다.
- browser 실행 중 다른 Rust 빌드가 target/debug binary를 교체하여 신규 형식 guard의 적용이 케이스마다 달라진 실패는 그대로 보존한다. 최종 검증은 `ARCHITALK_BROWSER_BINARY`로 실행별 고정 복사본을 사용한다. 이 환경변수는 브라우저 테스트 runner 전용이며 제품의 CLI 실행 설정이 아니다.

## PKG-INT-02 — 최종 변경 후 회귀

- 실행 전 범위: HTML block guard, 새 DSL 문법 예시, 0.2.0, ignored 실제 workflow의 10파일 단언까지 포함한다. `cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test --locked`를 순서대로 실행한다.
- 첫 실행: root가 추가한 real_workflow의 다중행 assert 서식으로 fmt exit1. clippy·test는 시작하지 않았다. 해당 파일에 rustfmt를 적용하고 같은 명령으로 재실행한다. 기능 실패나 테스트 통과로 세지 않는다.

- PKG-INT-02 재실행 결과: fmt exit0, clippy all-targets `-D warnings` exit0, `cargo test --locked` **103 passed / 5 ignored / 0 failed**. ignored 5건은 실제 CLI adapter 1, 구버전 binary 1, 실제 package 1, 실제 전체 workflow 2로 구분한다. 구버전 binary 실물 검사는 storage PKS-S04에서 별도 통과했고, 이번 실제 package 호출은 PKG-REAL-01에서 별도 기록한다. 새 전체 workflow 2건은 이 패키지 변경 후 실제 모델로 실행하지 않았다.

## PKG-INT-03 — 실제 패키지 생성 시간과 설정

- 실제 합성 10문서 design 호출이 180초를 넘어, 이전 기본 제한에서는 완성 응답 전에 중단될 수 있음을 관찰했다. 테스트 probe는 사전에 명시한 300초 제한 그대로 유지한다.
- 새 기본값과 설정 예시는 300초로 맞추고, 기존에 저장한 명시적 timeout은 덮어쓰지 않는다. 웹 설정에서 기존 지원 범위 10..600초를 직접 선택할 수 있도록 숫자 입력을 연결한다. 요청 중 run의 effective settings는 바꾸지 않으며 다음 작업에만 적용한다.
- 변경 후 계획: 설정 직렬화·범위·누락 보존 Node 계약, 설정 브라우저 desktop/mobile 표적, fmt·clippy·해당 HTTP/settings/workflow Rust 계약. 전체14가 검증한 package/보안 코드는 이 변경에서 수정하지 않는다.

## PKG-REAL-01 결과와 보존 응답 재검증 계획

- 실제 Codex(low, model=CLI default) 호출은 **251.84초**, 10종 합계 **46,446 UTF-8 bytes**를 반환했다. CLI가 terminal 응답을 반환했지만 주 문서의 module 절에 직접 DSL이 없다는 형식 검사로 **0 pass / 1 fail**, exit101이었다. `target/document-probes/3b881d77-4d6b-4109-8db1-29dacaa6b98f/response.json`에 원응답을 보존했다. 실패 전에 resolved model metadata를 저장하지 않았으므로 특정 모델명을 추정하지 않는다. 실제 호출은 1회다.
- root가 전체 주 문서 및 품질·대안·ADR·검증·잡플로우를 읽었다. 4개 품질 시나리오, 3개 대안 비교, 3개 proposed ADR, 4개 검증 기록을 포함하며 미정 수치와 측정·확정 조건, 손상 자료·중첩 요청·로컬 노출·복구를 분석한다. 이는 합성 프로젝트 한 건의 관찰이며 모든 분야의 의미적 정답으로 일반화하지 않는다.
- 실패 원인: main의 `Method-R module design`은 실제 `잡플로우.md#자료-조회와-검증`의 orchestrator DSL을 가리켰다. 기존 검사는 주 문서 내의 도표만 세어, 문서 분리·상세 링크·중복 방지 지침과 충돌했다. main의 macro scope 도표는 존재했다.
- PKG-REAL-02 예정: macro scope 도표 필수는 유지하고, module 도표는 주 문서 안 또는 **module 절이 연결한 실제 상세 heading 영역**의 유효한 orchestrator DSL을 허용한다. 링크가 없거나 다른 절의 scope 도표만 있거나 대상이 없으면 거절하는 회귀를 선행한다.
- 실제 모델을 재호출하지 않는다. `ARCHITALK_PACKAGE_RESPONSE_JSON=<보존한 합성 응답> cargo test --test real_document_format -- --ignored --nocapture`로 수정 검사를 동일 원응답에 적용하고, 원본을 수정하지 않은 새 artifact 폴더에 결과를 기록한다. replay는 provenance에 model_calls=0을 명시한다.
- 추가로 해당 원응답을 static localhost renderer API로 표시하는 opt-in browser 2건과 timeout 설정 desktop/mobile 2건을 실행한다. 실제 생성과 보존 응답 재검증, fixture의 전체 14건 결과를 각각 구분한다.

- PKG-INT-03 Node 재검증: 기존 API/SSE/설정/저장·신규 package 포함 **28/28 통과**. 새 제한 입력의 경계·정수·빈 값·누락 보존과 원본 설정 불변을 확인했다. UI는 설정 파싱도 기존 command 오류 처리 안에서 수행하여 잘못된 값이 요청되기 전에 표시한다.

## PKG-INT-04 — 모듈 문서 연결 수정 후 최종 조립 계획

- module 위임의 실제 heading 범위·orchestrator 및 실제 inline link 검사를 보강한 뒤 전체 조립을 최종 확인한다. fmt/clippy all-targets와 `cargo test --locked`, 보존된 실제 응답 replay, 고정 browser binary의 설정 2건·원본 생성문서 렌더 2건이 대상이다. 추가 유료 호출은 없다.
- raw 응답은 수정하지 않는다. 링크 계약의 정상 대체 표현을 받는 것과 가짜 링크·다른 heading·없는 도표를 거절하는 것을 함께 검증한다. 브라우저 기존 14건은 B03의 고정 binary 결과로 유지하고, 이 변경의 소비자인 generation/package domain은 Rust 회귀와 실제 응답으로 확인한다.

- PKG-INT-04 Rust 최종 결과: fmt exit0, clippy all-targets `-D warnings` exit0, 전체 **108 passed / 5 ignored / 0 failed**. Node는 timeout 포함 **28/28**이며 서버 형식·저장·복구·설정 소비자 회귀까지 통과했다.
- PKG-REAL-02 최종 결과: 강화된 실제 link/heading 범위 검사 후 동일 원응답 **1 pass**, 추가 오류 없음. 새 artifact는 `target/document-probes/a15680e0-4a40-4258-86b8-8afd1f4c5f1b/`, provenance model_calls=0이다. 중간 replay 및 최초 실패는 도메인 D09~D13과 위 PKG-REAL-01에 유지한다. 총 실제 모델 호출은 여전히 1회다.
- PKG-INSTALL-01 실행 시작: 위 core 검증 통과·제품 소스 동결 후 `./install.sh`로 release 바이너리를 전역 교체한다. 이어 새 로그인 셸에서 PATH 및 `architalk --version`을 확인한다. 기존 사용자 서버는 종료하지 않는다.

- PKG-INSTALL-01 결과: `./install.sh` exit0, release 빌드 7.45초, 전역 0.1.0→0.2.0 교체 완료. `/tmp`에서 새 로그인 셸의 PATH가 Cargo bin의 architalk로 연결되고 `architalk --version`이 **architalk 0.2.0**을 반환했다. 사용자 실행 중 서버와 기존 생성 문서는 수정·종료하지 않았다. README·implementation·검증 색인의 최종 로컬 링크도 모두 존재한다.

- root는 실제 응답 renderer의 desktop `architecture.png`와 mobile `screen_layout.png`를 직접 열어 시각 확인했다. metadata·callout·SmartMD 표·Method-R 업무 경계 도표와 문서 지도, 목록/상세/조회 중/빈 결과/목록 오류의 layout이 원문 fence 대신 표시되고 화면 밖으로 넘치지 않았다. 이 화면은 실제 사용자 앱을 건드리지 않은 static localhost 검증이며 app 종류 선택·저장은 별도 B03으로 검증했다.

## 최종 판정

- Rust **108 passed / 5 ignored**, Node **28 passed**, fmt/clippy 경고 없음. 기본 browser B03 **14/14**와 최종 변경 표적 **4/4**(실제 생성 원문 renderer desktop/mobile 2, 시간 제한 설정 desktop/mobile 2)가 통과했다.
- 실제 원문 10종·46,446 bytes·12개 도표가 각 viewport에서 오류 없이 표시됐고 자동 API/외부 요청·pageerror·page overflow가 없었다. 원본 응답은 변경하지 않았다. 최초 실제 호출의 형식 실패와 무과금 replay 통과를 분리한다. 추가 유료 호출은 없다.
- 설정 표적의 첫 실행은 PATCH 관찰자가 envelope.payload를 빠뜨린 테스트 코드 오류로 실패했다. `payload.settings`로 고친 뒤 numeric PATCH 480/240/120, invalid 시 PATCH 없음, 저장/새로고침, 현재 run 240 고정·다음 run 120을 검증해 **2/2** 통과했다. 이 실패와 최초 2/2 실제 renderer 통과는 [브라우저 기록](design-package-browser-20261118.md)에 보존한다. 제품 소스는 바꾸지 않았다.
- 실제 Codex 생성 1회와 그 원응답 검증/렌더링, fixture 전체 경로, 이전 CLI의 schema 거절을 확인했다. 새 패키지의 실제 Claude/Codex conversation→critic→저장 전체 유료 workflow 2건, Linux/Windows·전원 손실·장기 부하·실제 disk-full은 이번에 실행하지 않았다.
- README에 프로젝트 의도·드라이버부터 검증까지의 과정·Method-R·10종 결과물·새 초안 적용·설정과 사용 경로를 반영했다. 전역 설치 0.2.0과 프로젝트 밖 실행도 통과했다. 실행 중인 사용자 서버는 재시작하지 않았다.
