# 자동 클립보드 복사 복구 검증

## 사전 범위

- 기준 `591096a`(0.6.0), clean main에서 시작했다. 사용자 증상은 전사 완료 뒤 `자동 복사를 사용할 수 없어요` 안내다. 기존 코드는 브라우저 `writeText` 실패 이유를 버리고 수동 복사 창만 표시하므로 실제 환경의 거절 사유는 이 메시지만으로 확정할 수 없다.
- mdview는 완료 후 macOS `pbcopy`를 실행한다. 직접 클릭은 브라우저 복사를 먼저 즉시 호출하고, 실패 시 인증된 로컬 POST로 시스템 복사를 수행한다. 양쪽 실패만 수동 복사와 고정된 사유 안내로 이어진다. 클립보드 읽기·원문 저장·실제 사용자 설정 변경은 없다.
- backend 작성자와 browser 검증자를 분리한다. root는 viewer-menu/app 연결·README/API·정적/소비자 검증·설치를 맡는다. 모든 테스트는 임시 프로젝트, 가짜 음성/CLI/전사/네이티브 writer를 사용하며 실제 클립보드·마이크·유료 모델·API·사용자 서버를 건드리지 않는다.

## 실행 계획

| ID | 필수 판정 |
| --- | --- |
| CP-I01 | 직접 클릭의 브라우저 쓰기는 await 전에 실행한다. 성공이면 로컬 API를 부르지 않는다. |
| CP-I02 | 브라우저 실패 후 로컬 성공은 정확한 전체 지시문을 한 번 전달하며 성공 안내만 표시한다. |
| CP-I03 | 양쪽 실패는 원문을 보존한 수동 창과 원인 안내를 표시한다. 직접 복사 재시도·취소·포커스 복원을 확인한다. |
| CP-I04 | clear/dispose 뒤 늦은 브라우저 실패는 native 요청/모달을 시작하지 않는다. 진행 중 native 요청도 취소한다. |
| CP-I05 | native API의 권한/Origin/Host/프로젝트 유효성·입력 한도·명령 stdin·시간 제한/종료·무보존 계약을 대역으로 검증한다. |
| CP-I06 | Node 소비자와 관련 browser, Rust 검사, README/API, release 설치를 확인한다. 실제 OS 클립보드는 미실행이다. |

## 실행 기록

- 새 native 성공 반례를 이전0.6.0 바이너리에서 실행한 결과0 pass/1 fail로 browser-only 결함이 재현됐다. 상세 조건과 run ID는 [브라우저 이력](clipboard-browser-20261118.md)에 보존한다.
- root `node --check web/viewer-menu.js` exit0, `npm run test:web` **68 pass / 0 fail / 1 opt-in skip**,614ms. 이 시점 frontend는 browser-first/native-fallback 및 세대/abort 처리를 포함하고 backend 표적 검증은 담당자가 진행 중이다.
- backend 표적6/6, 기존 viewer API7/7, fmt/clippy(all-targets,-D warnings) 통과를 확인했다. root의 전체 `cargo test --locked`도 **191 pass / 0 fail / 12 ignored**다. child fifo probe의 중복 summary1개는 합계에서 제외했다. ignored는 기존 실 CLI/모델 및 opt-in 진단이다. 전체 stdout은 ignored `target/clipboard-rust-tests.log`에 보존한다.
- 고정 바이너리 `target/architalk-clipboard-fix`는0.6.1, SHA256 `4b15907f1616bc543ac3f40dfe08b826927f42f6d01638edf85a5026986e12b6`이다. 이 바이너리의 browser run `clipboard-fix-20261118`은 **20/20 pass**,25.9초, 재시도0이다. 음성16·파일/패널/저장4사례를 검증했다. 네이티브 API는 모든 사례에서 대역으로 막아 실제 클립보드에 쓰지 않았다.
- 직접 클릭의 브라우저 복사 성공에는 로컬 요청이 없고, 거절 후 로컬 성공은 원문과 동일한 전체 타임라인을 인증된 요청1회로 전달했다. 양쪽 실패의 전체 원문/허용 목록 안내/직접 재복사와 workspace410 동안 browser/native 각각 늦은 완료 차단을 확인했다. 모바일 수동 창의 버튼도 화면 안에 표시됐다. 상세 반례/화면 관찰은 [브라우저 이력](clipboard-browser-20261118.md), 프로세스/권한/무보존 검증은 [backend 이력](clipboard-backend-20261118.md)에 있다.
- README/API에 macOS 시스템 복사 대체·원인 안내·무보존을 반영했다. root `cargo fmt --check`, JS 문법과 `git diff --check` exit0, `cargo build --release --locked` exit0(8.33초)이다. 설치 후 다른 폴더의 PATH/버전과 커밋·푸시를 확인한다. 실제 마이크/유료 API/시스템 클립보드/사용자 서버는 미실행이다.
- backend 작성자의 read-only 최종 UI/API/문서 교차 검토에서 차단 결함은 없었다. `./install.sh` exit0으로 전역0.6.0을0.6.1로 교체했고, 다른 폴더의 새 login shell에서 Cargo bin의 `architalk --version`이0.6.1임을 확인했다. 사용자 서버를 종료하거나 실행 중 탭·설정은 변경하지 않았다. 기존 서버에는 재시작 후 새 코드가 적용된다.
