# 사이드바 배치와 작업공간 디자인 구현 기록

## 범위와 사전 계획

- 기준은 `090237d` (`v0.4.0`)이며 이 담당자는 `web/index.html`, `web/style.css`, `web/app.js`와 이 기록만 편집한다. Root가 회귀 테스트·버전·설치·커밋을 담당한다.
- 현재 파일 트리는 `margin-top:auto`인 `.sidebar-bottom` 안에 있어 설계 단계 아래에서 떨어진다. 트리를 단계 직후 독립 구역으로 옮기고 로컬 기록 안내와 프로젝트 경로만 하단 footer에 남긴다.
- 두 패널은 `overflow:auto`이며 접으면 `display:none`이므로 토글은 패널 밖의 형제 버튼으로 둔다. 고정 위치와 공유 CSS 폭 변수로 패널의 안쪽 경계에 붙이고, 닫으면 창 가장자리에서 복원한다. 최소 32px 폭·명확한 방향 아이콘·동적 접근성 이름을 제공하고 좁은 화면에서도 두 버튼이 겹치지 않게 overlay의 여유 폭을 확보한다.
- 기존 명시 grid column/row, <=1030px 왼쪽 overlay, <=760px 오른쪽 overlay, 작은 화면에서 한 패널만 열리는 상태 규칙과 ARIA/API는 유지한다. 헤더 74/68/66px와 탭·녹음 도구의 sticky 위치를 함께 유지한다.
- 사용자가 작업 중 전반적 디자인 개선을 추가했다. 기존 초록 브랜드와 기능을 유지하면서 중립 표면·읽기 쉬운 본문/보조 글자·일관된 간격/테두리/버튼을 적용한다. 헤더·프로젝트/대화/단계/파일·시작 화면·채팅/입력·문서/검토·메모·설정 dialog까지 같은 시각 체계를 사용한다. 채팅/문서의 유동 폭, 저장 dialog의 독립 scroll/footer, 기존 action과 서버 계약은 보존한다.
- `ux-ui-improvement` skill과 실행 프롬프트를 읽고 현재 코드의 관찰→개선→검증을 연결한다. 별도 UX 설계 문서는 Root 소유다. 외부 글꼴/이미지/의존성을 추가하지 않는다.

## 검증 계획

- 구현 후 `node --check web/app.js`, 기존 패널 상태 표적 Node 테스트, `git diff --check`로 구문과 상태 계약을 확인한다. 테스트 파일은 이 담당자가 변경하지 않는다.
- Root가 신규 실제 browser 회귀로 desktop/tablet/mobile의 경계 위치, 열기/닫기/스크롤/resize, 본문 폭과 가로 넘침, 파일 트리의 단계 직후 배치, 반응형·주요 화면을 확인한다.
- 사용자 DB/서버/전역 설정·실제 clipboard/microphone/provider를 호출하거나 변경하지 않는다. 소스 동결 후 Root에 구현 선택과 검증 범위를 전달한다.

## 구현과 표적 확인

- 파일 트리를 `.steps` 다음 형제 `.sidebar-files`로 분리했다. `.sidebar-bottom`은 로컬 기록 안내·경로만 가지며 기존 `margin-top:auto`로 하단을 지킨다. 단계 뒤 간격은 20px이고 파일 구역의 16px 상단 여백/구분선으로 위계를 표현했다.
- 헤더의 두 토글을 제거하고 `.app-body`의 고정 위치 형제로 옮겼다. 공유 폭 변수는 grid 폭·overlay 폭·토글 경계를 함께 정한다. 32×44px 버튼은 왼쪽 패널 오른쪽/오른쪽 패널 왼쪽에 있고, 닫으면 각 창 가장자리로 돌아온다. 76px의 좁은 화면 여유 폭으로 양쪽 32px 버튼 사이에 12px를 확보한다. z-index는 28로 헤더 30 아래·패널 25 위이며 focus outline은 안으로 그린다. 별도 resize/scroll listener를 추가하지 않았다.
- `aria-expanded`·기존 ID/action/state를 유지하고 `title`과 `aria-label`을 같은 동적 접기/펼치기 문구로 갱신한다. 방향 SVG는 상태 class에 따라 반전한다. 동작 JavaScript 변경은 이 접근성 속성 갱신에 한정한다.
- 기존 초록 브랜드의 중립 표면·테두리·음영·간격을 정리하고 헤더/사이드바/시작/채팅/문서/메모/설정에 일관된 가독성을 적용했다. 채팅은 desktop 15px/mobile 14px이고 보조 정보는 11–13px이다. 문서 본문에만 명시적 15px/mobile 14px 범위를 주어 뒤에 로드되는 `document.css` 및 기존 작은 `p` 스타일에 묻히지 않게 했다. diagram 코드·구조·전용 controls/layout은 수정하지 않는다.
- 저장 modal의 세 행 grid·독립 scroll/footer·낮은 높이 조밀 모드를 유지했다. 채팅/문서에 새로운 최대 폭을 두지 않았다. 외부 자산·의존성·네트워크 동작을 추가하지 않았다.
- 실행한 `node --check web/app.js`는 exit 0이었다. 기존 표적 `node --test --test-name-pattern='workspace sidebars|save preview keeps' tests/web.test.mjs`는 **2 pass / 0 fail**, exit 0, 약 147ms였다. `git diff --check`도 exit 0이었다. 실제 브라우저 시각/기하/상태 소비자는 Root가 이 동결 소스로 수행한다.
- 독립 정적 검토에서 커진 탭 행과 기존 녹음 도구의 45px sticky 간격이 어긋나는 문제가 발견됐다. 공유 `--workspace-tabs-height:51px`와 탭 행 높이·닫기 아이콘 12px를 명시하고 녹음 도구를 `header-height + workspace-tabs-height`에 두어 겹침을 제거했다. Root 요청으로 버튼/링크/summary/입력의 focus-visible을 진한 브랜드 초록으로 통일하고 가장자리 버튼의 안쪽 outline은 유지했다. 수정 후 구문·diff 검사는 다시 exit 0이었다.
- Source를 동결해 Root와 독립 reviewer에게 전달했다. Root의 새 실제 browser 소비자 결과나 concrete 결함이 올 때만 추가 제품 변경을 한다.
