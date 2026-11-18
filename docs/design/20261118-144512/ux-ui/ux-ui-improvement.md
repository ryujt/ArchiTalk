# ArchiTalk 작업 화면 개선

대상은 프로젝트를 대화로 설계하고 문서를 읽고 반영하는 기존 웹 화면이다. 파일 목록을3단계 바로 아래에서 찾고, 양쪽 패널의 경계에서 접고 다시 열 수 있도록 하며 대화·문서를 오래 읽기 편한 화면으로 정리한다. 서버 API와 대화·생성·전사·저장 정책은 바꾸지 않는다.

## 관찰과 근거

| 관찰 | 사용자 영향 | 개선과 확인 |
| --- | --- | --- |
| 파일 트리를 포함한 footer에 `margin-top:auto` 적용 |3단계와 파일 목록 사이가 화면 높이에 따라 벌어짐 | 파일 영역을 단계 다음의 일반 흐름에 두고 안내·경로 footer만 분리. 실제 두 영역 사이 거리 확인 |
| 양쪽 패널 제어가 header의 ‘파일/설계 메모’ 메뉴 | 패널의 열기·닫기 기능과 위치를 알아보기 어려움 | 왼쪽 패널 오른쪽 경계/오른쪽 패널 왼쪽 경계에 방향 버튼. 접힌 뒤에도 화면 가장자리에 복원 제어 유지 |
|9~11px의 옅은 안내·메타데이터와13px 대화 본문 | 문서와 상태·행동의 위계가 약하고 읽기 부담이 생길 수 있음 | 본문과 보조 글자 크기·대비를 높이고 일관된 제목/상태/행동 계층 적용. 실제 화면을 시각 점검하며 사용성 개선 정도는 별도 사용자 검증 대상 |
| 카드·버튼·입력창·tabs에 누적된 간격과 표면 표현 | 단계·대화·문서·설정의 화면 인상이 분산 | 기존 녹색 브랜드에 중립 배경과 흰 표면, 공통 간격·모서리·테두리·포커스 적용 |

조사는 기존 [화면](../../../../web/index.html), [스타일](../../../../web/style.css), [UI 조립](../../../../web/app.js)과 이전 [실행 이력](../../../../tests/test-history/viewer-workspace-20261118.md)을 사용했다. mdview 참고 작업에서 채택한 파일 탐색·문서 탭·녹음 기능을 유지한다. 다른 서비스의 실제 화면을 새로 벤치마킹한 것으로 주장하지 않는다.

2026-11-18 확인한 [W3C Target Size](https://www.w3.org/WAI/WCAG22/Understanding/target-size-minimum)는 작은 제어의 크기·간격을 다룬다. 이 화면의 패널 핸들은32px 이상의 클릭 영역을 사용하고 좁은 화면에서 서로 겹치지 않도록 한다. [W3C Focus Not Obscured](https://www.w3.org/WAI/WCAG22/Understanding/focus-not-obscured-minimum)는 작성자가 만든 고정 요소에 포커스가 가려지는 문제를 다룬다. 고정 header·녹음 표시·패널 핸들의 겹침을 실제 브라우저에서 확인한다. 이 두 검사는 전체 WCAG 적합성 인증을 뜻하지 않는다.

## 배치와 표현

기존 배치 개념:

```layout
Workspace V Header, Body
Header > Brand, GlobalPanelButtons, Settings
Body > LeftSidebar, Main, RightSidebar
LeftSidebar V Project, Conversations, Steps, EmptySpace, FilesAndPrivacy
Main V WorkspaceTitle, Tabs, Content
RightSidebar V ReferenceTabs, ReferenceContent
```

개선 배치 개념:

```layout
Workspace V Header, Body
Header > BrandAndProject, ConnectionAndSettings
Body > LeftArea, Main, RightArea
LeftArea > LeftSidebar, LeftBoundaryButton
LeftSidebar V Project, Conversations, Steps, ProjectFiles, PrivacyFooter
Main V WorkspaceTitle, DocumentTabs, ReadingTools, Content
RightArea > RightBoundaryButton, RightSidebar
RightSidebar V ReferenceTabs, ReferenceContent
```

위 도표는 위치 관계를 설명한다. 실제 핸들은 패널 스크롤/숨김 영역 밖에 둔 고정 제어이며 영구적인 화면 비율을 나타내지 않는다. 큰 화면에서는 양쪽 패널을 독립적으로 접고 펼치며 중앙은 남는 너비를 채운다. 작은 화면의 기존 overlay 동작과 한 번에 한 패널을 여는 규칙을 유지하고 양쪽 복원 제어를 동시에 찾을 수 있게 한다. 이동 API와 상태 전이는 동일하므로 새 Navigation/State 계약은 만들지 않는다.

환영 화면의 시작 입력, 대화 본문과 작성 중 메시지, 입력창, 문서 선택·녹음 도구, 검토와 설정·저장 dialog까지 같은 시각 규칙을 적용한다. 본문을 좁은 고정 최대 너비로 제한하지 않는다. 오프라인 내장 자산 원칙을 유지하고 외부 글꼴·CDN·새 그래픽 의존성을 추가하지 않는다.

## 적용 순서와 검증

1. 파일 목록/안내 footer를 분리하고 경계 버튼을 기존 ID·action에 연결한다.
2. 공통 색·글자·간격과 주요 컴포넌트 표현을 정리한다.
3. 작성자 외 검토자가 숨김/overflow/overlay·포커스·가독성 반례를 확인한다.
4. 실제 desktop/mobile와 중간 폭, 짧은 화면에서 전체 흐름·녹음 대역·파일 읽기·저장 버튼을 확인하고 PNG를 점검한다.

측정하지 않은 작업 시간 단축이나 만족도 수치는 주장하지 않는다. 최종 구현과 통과/실패·수정·미실행 범위는 [검증 이력](../../../../tests/test-history/workspace-redesign-20261118.md)에 남긴다. 어두운 테마나 서버 기능 확장은 이번 개선 범위에 추가하지 않는다.
