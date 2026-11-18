# 설계 생성 시간 제한 개선 — 2026-11-18

## 원인과 범위

- 사용자 프로젝트는 읽기 전용 SQLite metadata만 확인했다. 대화 작업은 정상 완료됐고, design run은 적용된 제한300초에 300.327초 만에 timeout으로 종료됐다. 종료는 confirmed, 메시지6개와 초안0개 상태였다. 원문·응답·인증 정보를 기록하지 않는다. 공용 기본 설정 파일은 없었다.
- 10종 문서 생성에도 동일한 기본300초·설정상한600초를 적용해 충분히 기다릴 수 없었다. stderr의 `reason=none, component=unknown`은 추가 원인이 아니라 비어 있는 진단이었다.
- Root/Architect는 단일 요청 제한의 기본값1800초(30분), 최대3600초(1시간)를 선택했다. config/UI 최소10초, adapter 최소1초는 기존 짧은 프로세스 회귀를 위해 유지한다. 역할별 새 설정·무제한 대기·자동 재실행은 추가하지 않는다.
- 명시된 기존 TOML 값은 보존한다. 파일 또는 필드가 없을 때 새 기본값을 사용하며 프로젝트의 과거 run 설정·메시지·초안은 바꾸지 않는다. 현재 사용자의 DB·서버·공용 설정은 수정하지 않는다.
- 역할: Backend Developer(config/adapter/프로세스 회귀), UI Developer/Tester(한국어 오류·설정·실제 브라우저), 독립 Critic(계약·반례), Root(실제 원인 metadata·App 호환 테스트·문서·설치·커밋/푸시). 기존 에이전트의 상속 모델을 유지하며 실제 모델 호출은 없다.
- Critic 검토: 고정 deadline/설정 상속/다음 run 적용/취소·종료미확인 차단을 유지하는 조건으로 동의. 자동 critic이 다른 프로젝트의 공용 변경을 자체 refresh하지 않는 기존 경계는 이번 고정 timeout 원인과 분리해 보류하며 App 코드는 변경하지 않는다.

## TI-01 사전 검증 계획

- `tests/timeout_app.rs`: 이전 DB에300초 설정과 실패 design 이력이 있어도 공용 파일이 없으면 main과 같은 `App::open_with_shared_config`에서 새 기본값1800초를 사용하고 모든 세션/run 내용을 유지하는지 검사한다. 명시된 공용300초 설정이 있으면 그대로 유지하며 파일/과거 run을 다시 쓰지 않는지도 별도로 검사한다. 실제 모델은 호출하지 않는다.
- Backend의 상한/기본값·명시값 보존·오류 적용초·빈 진단 및 기존 짧은 timeout/cancel 회귀는 [backend 기록](timeout-backend-20261118.md)에 보존한다.
- UI의 기존/새 범위, 현재 run과 새 설정 구분·한국어 설명·초안 재생성·입력/초안 보존과 실제 App10초 timeout의 desktop/mobile 복구는 [UI 기록](timeout-ui-20261118.md)에 보존한다. 제품 최소 제한을 낮추거나 paid generation을 호출하지 않는다.
- 소스 동결 뒤 fmt/clippy, 전체 Rust·Node, 관련 브라우저 표적 결과를 확인한다. README/예시/구현 안내에 실제 시간·요청별 기준·명시값 보존·재시도 방법을 반영한다.
- 통과 후0.2.3을 전역 설치하고 프로젝트 밖 login shell의 버전을 확인해 커밋·푸시한다. 현재 사용자 서버는 자동 중지하지 않는다.

### TI-01 수정 전 기준선

- `cargo test --test timeout_app -- --nocapture`: 1 PASS / 1 FAIL(exit101, 0.04초). 파일이 없는 기존 workspace의 실제 main App 경로는 여전히300초여서 기대1800초와 달랐다. 명시300초 보존 반례는 이미 통과했다. 사용자 데이터나 모델 호출은 없다.
- Root의 App 테스트와 README/설정 예시/구현 안내를 작성자 외 Critic이 읽기 전용 검토했다. blocker는 없었고, 설정 예시의 `next request`를 `next new task`로 좁혀 진행 중 작업의 형식 보정도 기존 snapshot을 사용한다는 지적을 반영했다. 파일이 존재하되 timeout 필드가 없는 경우는 Backend 계약 테스트가 담당한다.

### TI-02 구현 후 App 및 최종 검증 계획

- Backend 소스 동결 후 `cargo test --test timeout_app -- --nocapture`: 2/2 PASS(exit0, 0.05초). 새 기본값 적용과 명시300초 유지 모두 기존 세션/run·프로젝트 문서 설정을 그대로 보존한다. 실제 모델 호출은 없다.
- Backend/UI production 소스가 동결됐고 Root가 diff를 직접 확인했다. `cargo fmt --check`, `cargo test --locked`, 전체 Node 두 suite, `cargo clippy --all-targets -- -D warnings`, `git diff --check`를 실행한다. Browser의 실제10초 timeout/명시 복구는 UI 담당자의 isolated 표적 결과로 확인한다.

### TI-02 최종 정적·회귀 결과

- 0.2.3 `cargo test --locked`: 149 PASS / 11 ignored / 0 FAIL(exit0). 실제 모델·설치 CLI 추가 진단·구버전 바이너리 전용 테스트는 기본 실행에서 제외했다.
- `node --test tests/web.test.mjs tests/design-package-ui.test.mjs`: 38/38 PASS(exit0, 약552ms). 기존 Node module type 추론 안내는 그대로이며 실패가 아니다.
- `cargo fmt --check`, `git diff --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS(exit0, 3.55초).
- 작성자 외 Critic이 최종 Backend/UI와 새 반례를 읽기 전용으로 검토했다. 고정 deadline·실행 설정 snapshot·cancel/kill/drain·종료 미확인·진단 분류·명시적 재생성/내용 보존에 blocker가 없었다. 프로세스 부분 출력 뒤 실제1초 timeout으로 실패하고 종료되는 검증도 유지했다.

### TI-03 브라우저 및 전역 설치 계획

- UI 담당자가 동일 소스의0.2.3 고정 binary로 신규 실제 timeout desktop/mobile2건과 기존 timeout 설정 desktop/mobile2건을 실행해4/4 PASS(52.9초, retries0)했다. 실제 App10초 제한으로 실패·종료를 확인한 뒤1800초 저장→명시 재생성→design/critic 완료·이전 초안/메시지 보존까지 확인했다. 이는 실제 모델이 아닌 synthetic CLI 경계 검증이다. 구체적인 artifact/SHA/최초 반례/시각 확인은 UI 기록에 보존한다.
- 이 소스로 `./install.sh`를 실행하고 프로젝트 밖 `/tmp` login shell에서 전역 `architalk --version`을 확인한다. 현재 사용자 서버를 종료하거나 실제 생성 작업을 재실행하지 않는다.

### TI-03 최종 설치 결과

- `./install.sh`: release build7.38초, 전역 바이너리0.2.2→0.2.3 교체(exit0). 프로젝트 밖 `/tmp` 새 login shell의 `architalk --version`은 `architalk 0.2.3`(exit0)이었다.
- UI 담당자는 desktop timeout 안내와 mobile 설정 focus의 PNG를 실제 열어 확인했고 전체18 PNG/2 boundary JSON을 test artifact에 보존했다. 브라우저4건과 Node38건의 source/test/history를 최종 동결했다.
- 마지막 `git diff --check` PASS. 설치 중 현재 사용자 서버/DB·공용 AI 설정은 변경하지 않았다. 사용자는 이전 서버를 종료하고 새 바이너리로 실행한 주소에서 기존 대화를 선택해 `초안 다시 생성`을 명시적으로 누른다. 명시된 기존 짧은 제한을 유지하는 환경이라면 `시간 제한 조정`에서 먼저 변경한다.
