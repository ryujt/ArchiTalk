# 작업 화면 전체 디자인과 사이드바 경계 검증

## 사전 범위·분업

- 기준 `090237d`(0.4.0), 작업 시작 시 clean main이다. 사용자의 파일 목록 위치/양쪽 접기 버튼 개선에 전체 디자인 개선을 추가했다. Developer는 HTML/CSS/패널 표시만, 별도 Critic은 실제 배치 반례를, root는 UX 근거·브라우저/통합 검증·README·설치/커밋을 맡는다. 모델 등급은 별도 변경하지 않았다.
- 기존 프로젝트 대화·AI·전사·저장 계약은 유지한다. 사용자 서버/DB·실제 마이크·클립보드·유료 모델/API를 쓰지 않는다. 실제 브라우저는 임시 프로젝트와 copied CLI fixture만 사용한다.
- 관찰: 파일 트리는 `.sidebar-bottom`의 auto margin 때문에 단계와 떨어지고, 양쪽 제어는 header 안의 메뉴로만 노출된다. 작은 옅은 글자와 누적된 컴포넌트 간격도 전반적인 가독성 개선 대상이다.

| ID | 검증할 실제 결과 |
| --- | --- |
| WR-E01 | 프로젝트 파일 제목이3단계 바로 아래에 위치. header 메뉴 대신 각 sidebar의 안쪽 경계에 제어가 있고 접은 뒤에도 복원 버튼이 화면에 남음 |
| WR-E02 | desktop/tablet/mobile/낮은 화면과 스크롤 후 양쪽 버튼을 실제 클릭·키보드 조작 가능, 서로 가리지 않고 main 폭/가로 넘침 정상 |
| WR-V01 | welcome/chat/review/document/settings/녹음/저장의 글자·표면·버튼·간격을 실제 PNG로 점검. fluid 대화폭과360px 저장 footer 회귀 방지 |
| WR-I01 | Node 소비자와 관련 browser 흐름 재검증, 버전/README/전역 설치·커밋·푸시 확인 |

## 실행 전 계획

새 `tests/browser/sidebar-edge.spec.mjs`로 baseline0.4.0의 header 제어/하단 tree 문제를 먼저 관찰한다. 이후 변경 소스의 frozen binary에서 desktop/mobile 및 중간 폭·스크롤 geometry를 확인한다. 전체 CSS를 변경하므로 기존30 browser, voice8, viewer workspace4와 file race4를 최종 UI에서 확인하며 코드/네트워크 대역과 시각 QA를 구분한다. 실패·검증자 보정·제품 수정은 아래에 보존한다.

## 실행 기록

- baseline 명령: `ARCHITALK_BROWSER_BINARY=target/architalk-viewer-fixed ARCHITALK_BROWSER_RUN=workspace-redesign-baseline-20261118 npx playwright test tests/browser/sidebar-edge.spec.mjs --project=desktop -g 'project files'`. 결과는 **0 pass / 1 fail**, exit1. 실제 header 제어가2개여서 새 배치 기준0개에 실패했다. 기존 화면 PNG를 artifacts에 보존했다. 제품 코드를 우회해 기준을 맞추지 않았다.
- 전체 시각 개선의 [근거와 배치 설계](../../docs/design/20261118-144512/ux-ui/ux-ui-improvement.md)를 작성했고 version0.5.0으로 올린다. backend/DB/AI 제한은 변경하지 않는다. 기존 workspace browser의 고정 aria-name click은 동일 `#info-toggle`로 바꾸어 동적인 열기/닫기 이름을 허용하며 새 테스트에서 가시성과 실제 클릭 위치를 별도로 확인한다.
- Critic은 글자·tabs 높이가 커지면서 기존45px 녹음 도구 sticky offset과 겹치는 구체적인 문제를 발견했다. Developer는51px 탭 높이를 공통 변수로 고정하고 녹음 도구도 같은 높이를 사용하게 수정했다. root는 파일 탭을 연 채 스크롤할 때 `tabs.bottom <= recorder.top`의 실제 geometry를 새 반례에 포함한다.
- 소스 동결: HTML/CSS/app panel labels만 변경했다. Developer의 구문·기존 패널/저장 Node2·diff check가 통과했고 별도 Critic 정적 재검토에 남은 blocker는 없었다. root는 새 frozen0.5.0 build에서 Node 전체와 신규 배치4사례부터 확인하고, 통과 후 주요 UI 전체 브라우저를 검증한다.
- `cargo check --offline`, `cargo build --locked --bin architalk`, `cargo fmt --check` exit0. `npm run test:web`는 **60 pass / 0 fail / 1 opt-in skip**,606ms였다. backend 소스 변경이 없어 기존 Rust180 사례를 의미 없이 다시 반복하지 않으며 전체 UI/소비자와 정적 조립을 확인한다.
- 고정 바이너리 `target/architalk-workspace-redesign`,0.5.0, SHA256 `cc31a1ee104bf1a7cd3caec549ff37ed31350f7c9edac24c5a69c83527c92381`에서 edge4와 기존 주요 UI46을 분리 실행한다. 기록 ID는 `workspace-redesign-edges-20261118`, `workspace-redesign-flows-20261118`이다. 실제 AI·마이크·OpenAI·클립보드는 사용하지 않는다.
- edge 결과는 **4/4 pass**, exit0,5.9초였다.1365/1024/768/390/320px, 패널 안쪽 경계/접힌 복원 제어의 실제 hit test, 파일 영역 간격, 스크롤/키보드, 본문 폭·가로 넘침과 tabs/recorder 비겹침을 확인했다. root가 desktop welcome 및 mobile overlay/320px 파일 화면 PNG를 직접 열어 글자·카드·목록·경계 버튼의 실제 표시를 확인했다. `cargo clippy --all-targets -- -D warnings`도 exit0.
- Critic도 같은3개 PNG를 별도로 열어 데스크톱·모바일의 파일 목록·글자·여백·양쪽32px 핸들·초점·긴 경로 줄바꿈을 확인했고 구체적인 시각 회귀를 찾지 못했다. 이 검토에서 제품 소스는 수정하지 않았다. 전체 흐름 검증과 별도로 root가 desktop 대화/설정 PNG도 직접 점검한다.
- `cargo build --release --locked` exit0,7.28초. 설치는 전체 UI 검증 완료 후 수행한다.
- 전체 주요 흐름 결과는 **46/46 pass**, exit0,3.5분이었다. 새 edge4와 합쳐 최종 서로 다른 browser는 **50/50 pass**다. 대화 최신 위치/작성 중, 모델 설정/시간 제한, 문서 단계/정화/도표, 파일 탭/경로 복사/복구, 음성 대역/비밀 설정,360px 저장 버튼/실제10파일 반영을 같은0.5.0 바이너리에서 확인했다. root는 모바일 녹음 결과와360px 저장 dialog PNG도 직접 열어 제어·본문·footer 가시성을 확인했다. 실제 사용자 프로젝트·기기·유료 서비스는 사용하지 않았다.

## 배포 확인 계획

README와 구현 안내에 새 배치·전체 시각 규칙을 반영했다. `./install.sh`로 전역0.5.0을 설치하고 다른 폴더의 새 login shell에서 PATH와 버전을 확인한 뒤 커밋·푸시한다. 사용자 서버는 종료하지 않으며 실행 파일에 포함된 새 자산을 보려면 재시작이 필요하다.

- `./install.sh` exit0, 전역0.4.0을0.5.0으로 교체했다. `/tmp`의 새 login shell에서 `architalk --version`은 `architalk 0.5.0`, PATH는 기존 Cargo bin의 실행 파일로 확인했다. 실제 프로젝트 서버나 DB를 열지 않았다. 최종 diff check도 exit0이며 테스트 binary/PNG는 target에만 보관하고 Git에 포함하지 않는다.
