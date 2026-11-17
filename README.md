# ArchiTalk

프로젝트 폴더에서 실행하고 웹 대화로 요구사항과 소프트웨어 아키텍처를 정리하는 Rust CLI입니다. 로컬에 설치한 Claude Code 또는 Codex를 사용하고, 문서는 프로젝트의 `docs/architecture/` 아래에 남깁니다.

## 빠른 시작

현재 AI 실행을 검증한 환경은 **macOS**입니다. 지원 프로필은 Claude Code **2.1.326**, Codex CLI **0.169.0**이며 둘 중 사용할 도구 하나를 설치하고 먼저 로그인하세요. 다른 버전은 `doctor`에서 미지원으로 안내합니다. 로컬 CLI를 사용해도 모델 처리는 해당 도구의 서비스·인증·이용 조건을 따릅니다.

```bash
./install.sh
cd /path/to/your-project
architalk
```

`install.sh`는 소스 체크아웃의 위치를 기준으로 빌드하므로 어느 폴더에서 실행해도 됩니다. `sudo` 없이 Cargo의 기본 설치 경로에 설치하며, 보통 `~/.cargo/bin/architalk`가 됩니다. Cargo의 `install.root`, `CARGO_INSTALL_ROOT`, `CARGO_HOME` 설정도 따릅니다. 같은 명령으로 재설치·업데이트할 수 있습니다.

설치 경로를 지정하려면 `./install.sh --prefix /원하는/경로`를 사용하세요. 실행 파일은 해당 경로의 `bin/architalk`에 설치됩니다. 설치된 `bin` 폴더가 `PATH`에 없다면 셸의 `PATH`에 추가한 뒤 새 터미널을 여세요. 스크립트는 셸 설정 파일을 자동 수정하지 않습니다.

브라우저가 열리면 프로젝트를 소개하고 질문에 답하세요. 모델·생각 수준은 **도구 설정**에서 선택합니다. **다음 단계로 · 초안 만들기**를 누르면 질문을 모두 마치지 않아도 현재 내용으로 초안 작성과 비판 검토를 진행합니다. 답하지 않은 항목은 미정으로 남습니다. 검토한 결과를 **초안으로 저장**하거나 준비 조건을 충족한 버전을 **검토본으로 확정하고 저장**할 수 있습니다.

응답을 기다리는 동안 대화 화면에 작성 중 메시지가 표시됩니다. 답변이 완료되면 최신 대화와 입력창이 보이도록 이동하며, 넓은 화면에서는 대화도 작업 영역의 너비에 맞춰 확장됩니다.

```bash
architalk --no-open                  # URL만 출력
architalk --project /path/to/project # 실행 폴더를 명시
architalk --port 8087                # 필요할 때 포트 지정
architalk doctor                     # 설치·버전·옵션만 확인, 모델 호출 없음
architalk --config /path/to/config.toml
```

기본은 현재 폴더이며 Git 루트로 자동 이동하지 않습니다. 종료는 Ctrl+C입니다. 자동 브라우저 열기가 실패해도 터미널의 URL로 사용할 수 있습니다. URL에는 이번 프로세스의 로컬 접근 토큰이 있으므로 공유하지 마세요.

## 모델과 생각 수준 설정

설정 파일은 `$XDG_CONFIG_HOME/architalk/config.toml`, 환경변수가 없으면 `~/.config/architalk/config.toml`입니다. [설정 예시](config.example.toml)를 복사하거나 `--config`로 경로를 지정하세요. 파일이 없어도 기본 설정으로 시작합니다.

- Claude는 `-p`, Codex는 `exec`로 비대화형 호출합니다. Codex의 `-p`는 프로필 옵션이며 현재 격리 프로필에서 named profile은 지원하지 않습니다.
- 기본 도구·모델·생각 수준과 대화/설계 작성/비판 검토 역할별 설정을 선택할 수 있습니다. 모델과 생각 수준은 알려진 값 목록에서 고르거나 바로 아래 입력란에 직접 입력하세요. 역할의 목록은 선택한 도구를 따르며, 도구를 바꿔도 입력한 값은 유지합니다. 기본 설정의 빈칸은 CLI 기본값, 역할 설정의 빈칸은 선택한 도구의 기본 설정을 사용합니다.
- 목록은 알려진 값의 예시이며 계정·CLI·모델별 지원을 보장하지 않습니다. 목록에 없는 모델 ID나 생각 수준도 직접 입력할 수 있습니다. 생각 수준은 영문·숫자·`_`·`-`로 최대 32자이며 `-`로 시작할 수 없습니다.
- 변경은 다음 작업부터 적용하고 현재 작업의 설정은 유지합니다. 선택한 모델이 생각 수준을 지원하지 않으면 오류를 표시하며 자동으로 다른 값으로 바꾸지 않습니다.
- 실행파일 경로는 신뢰된 사용자 설정 파일에서만 바꿀 수 있습니다. 웹은 임의 명령·환경변수·도구 권한을 추가하지 못합니다.
- 설치 진단은 로그인이나 특정 모델 지원을 검증하지 않습니다. 실제 AI 작업에서 인증 오류가 나면 해당 CLI의 터미널에서 처리한 뒤 명시적으로 재시도하세요.

## 문서와 대화 기록

대화·근거·설계 버전·실행·저장 상태는 프로젝트의 `.architalk/workspace.db`에 보관합니다. `.architalk/`를 Git 제외 목록에 추가하세요. 앱은 기존 `.gitignore`를 자동 수정하지 않습니다.

출력 예시는 `docs/architecture/회원-알림-서비스/아키텍처-설계.md`입니다. 다음 저장은 `회원-알림-서비스-개정-2/` 같은 새 폴더를 사용하고 기존 문서를 덮어쓰지 않습니다. 저장 전에 경로·파일·내용을 확인할 수 있습니다. 이전 초안은 당시 버전을 표시해 저장하며, 파일 저장과 내용 수락은 구분합니다.

새 설계 문서는 SmartMD 형식으로 네비게이션(`navigation`), 객체 협력 잡플로우(`jobflow`), 상태 전이(`state`), 화면설계(`layout`)를 포함합니다. UI가 없는 분야는 화면 관련 항목에 비적용 이유와 CLI/API 등 대체 인터페이스를 명시합니다. SmartMD 메타데이터·콜아웃·표와 전용 DSL을 문서에 보존하므로 저장한 파일을 `mdview`로 열어 볼 수 있습니다. [생성 형식 규칙](prompts/design-document-format.md)을 따르며, 과거에 저장한 문서는 원문을 유지합니다.

프로젝트 자료는 선택한 README·구성 파일의 제한된 텍스트만 전달합니다. 숨김/인증 파일·symlink·바이너리는 제외합니다. 질문에는 모름·추천·보류로 답할 수 있고, 확인되지 않은 값은 확정 사실로 처리하지 않습니다. 생성된 해석·요구는 근거와 함께 검토하세요.

프로세스가 중단되면 마지막 확정 상태를 복원합니다. 이전 AI 프로세스의 종료가 불명확하면 새 실행을 막고 직접 정리한 뒤 확인하도록 안내합니다. 모델 작업은 자동 재실행하지 않습니다. 문서 저장은 파일 hash와 manifest로 복구하며, 예상 밖 외부 편집은 충돌로 남깁니다.

## 개발과 검증

소스 빌드에는 Rust stable과 C 컴파일러(SQLite 빌드)가 필요합니다. 검증 환경은 Rust 1.99.1이며, 프런트엔드 검증에는 Node.js 20 이상을 사용합니다. 제품 실행에는 Node·별도 웹 서버·프런트엔드 빌드가 필요 없습니다.

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
node --test tests/web.test.mjs
cargo build --release --locked
```

브라우저 자동 검증은 별도의 개발 의존성입니다. 설치된 Chrome의 새 headless 프로필과 임시 프로젝트만 사용하며 기존 사용자 탭에 연결하지 않습니다. 실제 모델을 호출하지 않는 명시적 CLI fixture로 화면을 검증합니다.

```bash
npm ci
npm run test:browser
# Chrome이 없는 환경: npx playwright install chromium
# ARCHITALK_BROWSER_CHANNEL=chromium npm run test:browser
```

일반 테스트는 모델을 호출하지 않습니다. 설치된 CLI와 실제 모델을 사용하는 검증은 별도로 실행하며 사용량이 발생할 수 있습니다.

```bash
cargo test --test ai_adapter real_ -- --ignored --nocapture --test-threads=1
cargo test --test real_workflow -- --ignored --nocapture --test-threads=1
cargo test --test real_document_format -- --ignored --nocapture # Codex 설계 1회
```

두 번째 명령은 실제 바이너리→로컬 HTTP/SSE→CLI 대화·설계·검토→문서 저장→재시작을 임시 프로젝트에서 확인합니다. [검증 실행 이력](tests/test-history/README.md)에서 성공·실패·미실행 범위를 구분합니다.

| 모듈 | 책임 |
| --- | --- |
| [app](src/app.rs) · [model](src/model.rs) | 상태 변경·버전·중복 명령·작업 조율 |
| [domain](src/domain.rs) | 질문 선택·근거·수정 이력·검토 게이트 |
| [ai](src/ai.rs) | Claude/Codex 호출·출력 해석·취소·시간/크기 제한 |
| [storage](src/storage.rs) · [files](src/files.rs) | SQLite·잠금·고정 디렉터리 경계·자료 탐색·새 문서 저장 |
| [http](src/http.rs) · [web](web/app.js) | 인증된 로컬 API/SSE·실제 웹 UI |

## 지원 경계

AI 작업은 프로젝트 밖 임시 디렉터리에서 실행하고 macOS `sandbox-exec`로 원본 프로젝트 쓰기를 차단합니다. 같은 사용자 홈 전체를 격리하는 도구는 아닙니다. 관리형 CLI 정책을 검증할 수 없거나 필요한 제한을 적용할 수 없으면 실행을 거부합니다. 현재 릴리스 대상은 macOS와 로컬 파일시스템이며 Linux·Windows 빌드/AI 실행, 네트워크 공유 폴더, 임의 named profile은 아직 지원하지 않습니다.

부모 프로세스의 강제 종료·전원 손실·임의로 분리된 프로세스까지 정리를 보장하지 않습니다. 중단 뒤 종료 미확인 게이트를 유지합니다. 원격 공동 편집·코드 구현/수정·전체 저장소 무제한 탐색은 제공하지 않습니다. Markdown은 앱에서 실행되지 않는 안전한 텍스트로 표시합니다. 기본 진단은 stderr에 제한된 단계·오류만 기록하고 프롬프트·응답 전문·인증 정보를 로그로 남기지 않습니다.

[실제 구현/API 안내](docs/implementation.md) · [TO-BE 설계](docs/design/20261117-190055/to-be/system-design-to-be.md) · [화면 흐름](docs/design/20261117-190055/to-be/navigation.md) · [화면 배치](docs/design/20261117-190055/to-be/screen-layout.md) · [초기 화면 모형](docs/design/20261117-190055/to-be/details/screens/workspace.html)
