# 저장 창·파일 작업공간·문서 녹음 UI 검증

## 사전 범위와 읽기 전용 조사

- 기준 `e7bcb0c`(0.3.0). 이 담당자는 `web/app.js`, `web/style.css`, `web/index.html`, 새 file-viewer/document-recorder/viewer-menu 모듈, 관련 web/browser tests와 이 기록을 소유한다. renderer/document.css와 파일/secret/전사 API는 별도 담당자 소유다. wire 계약 동결 전 종속 코드는 편집하지 않는다. 독립 저장 modal/sidebar UI만 사전 반례와 계획 뒤 진행할 수 있다.
- 사용자 증상: 전체 문서 저장 modal에10개 원문이 펼쳐져 실행 버튼을 찾기 어렵다. source상 `saveModal`은 각 `.plan-file`에 `<pre>`를 직접 삽입하고 footer가 같은 scroll body의 끝에 있다. 파일 목록을 기본으로 보이고 본문은 닫힌 details로 분리하며, save modal만 header/scroll body/footer로 나누면 최소 경계에서 해결된다.
- 읽기 전용 참고: mdview의 `sidebar.js`는 lazy1level tree와 active file 표시, `app.js`는 파일 전환 순서 guard와 페이지 메모리 scroll을 사용한다. 조사한 현재 source에는 복수 tab/context menu가 없다. recorder는 open/copy/focus/word를 elapsed-ms+file/line 정보로 수집하고 stop 후 서버에 업로드한다. 현재 mdview의 getUserMedia 선행 흐름은 새 요구와 달라 서버 `has_api_key` 확인이 먼저 필요하다. 참고 repo는 수정하거나 실행하지 않았다.
- 보안/수명: 파일 본문·탭·프로젝트 tree·녹음/전사 결과는 페이지 메모리에만 둔다. 인증 무효화/프로젝트 reset/pagehide 때 media tracks·녹음·업로드·file fetch와 지연 응답을 중단/정리한다. AI 작업/대화 메시지를 자동으로 만들지 않는다. 실제 사용자 DB/서버/공용 브라우저 storage·실제 모델·실제 마이크·오디오 API를 조작/호출하지 않는다. 테스트는 fake media와 임시 프로젝트만 사용한다.

## 검증 계획

| ID | 범위 | 필수 판정 |
| --- | --- | --- |
| VW-N01 | 독립 저장 modal/패널 |10개 원문 기본 접힘, 파일명/개수 표시, footer가 독립 영역, 좌우 collapse/reopen 상태·ARIA와 항상 보이는 restore controls |
| VW-B01 | 실제 save desktop/mobile/낮은 높이 | 큰10문서에서도 footer와 실행 버튼이 viewport 내 접근 가능, 단일 body scroll, 정상 저장/취소/focus와 가로 넘침0 |
| VW-N02 | 파일 tabs/tree/menu | 정규화 서버 path 중복 탭 재사용·닫기·chat 유지, 늦은 응답/close/reset 차단, 경로/이름 copy와 수동 fallback, 공용 browser storage 쓰기0 |
| VW-N03 | fake recorder | 키 없음은 getUserMedia 전에 오류, 녹음 이벤트의 elapsed/file/heading/selection, stop 요청1회·실패/abort/clipboard 수동 fallback, 모든 track 해제 |
| VW-B02 | 새 UI 소비자 | 임시 tree/file API와 fake media로 탭·패널·상대 링크·복사·녹음/전사 흐름, auth/reset cleanup, pageerror/외부 요청/AI 명령0 |

- root backend wire와 renderer file callback 계약을 동결한 뒤 종속 기능을 연결한다. 새 정적 모듈 URL은 `/file-viewer.js`, `/document-recorder.js`, `/viewer-menu.js`다. Renderer 협의는 `currentFile`, `fileLinks`와 `onFileLink({path,anchor})`; 기존 package 링크 계약은 유지한다.
- 새 browser 실행은 fresh build를 고정 binary에 복사하여 SHA/버전과 실행 전 명령을 기록한다. 테스트 실행의 실패/중단과 수정·재검증을 모두 별도 남기고 필요한 표적 통과 뒤 불필요한 전체 반복은 하지 않는다.

## 실행 기록

### VW-N01 — 독립 modal/sidebar 반례 계획

- wire 동결 대기 동안 Node 반례로 저장 본문이 닫힌 details 안에 있고 footer가 scroll body 밖인 구조를 확인한다. 좌우 패널은 page 메모리 상태로만 접고 펼치며 controls의 aria-expanded를 갱신한다. CSS는 desktop grid column을 실제 제거하고 mobile에서는 닫힌 drawer가 공간을 차지하지 않게 한다. restore controls는 sticky header에 남긴다.
- 먼저 기존 모듈 Node harness에 반례를 추가해 `node --test --test-name-pattern='save preview keeps|workspace sidebars' tests/web.test.mjs`를 실행한다. 그 다음 독립 UI hunk만 수정한다. 아직 신규 wire/API/마이크/전사 연결은 구현하지 않는다.

- VW-N01 결과: 수정 전 **0 pass / 2 fail**, exit1, 약147ms. save scroll body/footer 분리와 좌우 메모리 상태가 없음을 확인했다. 독립 modal/sidebar 변경 뒤 동일 표적은 **2 pass**, exit0, 약154ms였다. `node --check web/app.js` exit0. 기존 원문은 파일별 닫힌 details에만 두고 save dialog를3행 grid로 만들었다. 패널은 desktop grid column 제거/mobile drawer로 접고 header controls를 유지한다.

### VW-WIRE — 확정 API 및 종속 구현 계획

- root/backend 계약: GET `/api/v1/files/tree`는 재귀 entries와 전체 bounded `files` catalog·root_display·truncated를, `/files/read?path=`는 `{path,name,kind,content,size,hash,absolute_path}`를 반환한다. Markdown과 text만 보며 HTML은 text, PDF/binary는 제외한다. 실제 filesystem Unicode 표기를 보존하므로 NFC 동등 매칭 뒤 **catalog 원본 path**를 read 요청에 쓴다.
- GET `/transcription/settings`는 configured/model/max_audio_bytes/timeout_seconds만, PATCH `/transcription/key`는 write-only api_key/model을 받고 같은 DTO만 반환한다. key 누락이면 media 호출 전에 오류를 표시한다. POST `/transcription`에는 multipart audio만 보낸다. 문서 click/selection/file/heading/timestamp는 서버 전송 없이 메모리에서 전사문과 지시문을 결합한다. backend20MiB/120초·단일 요청이며 자동 재시도하지 않는다.
- renderer는 별도 담당자가 `currentFile`, `fileLinks`, `onFileLink`와 pure resolver를 추가했다. 경로/anchor allowlist는 서버 catalog를 쓰고 기존 package/sanitizer/CSP 계약을 유지한다. UI는 이 source를 수정하지 않는다.
- 신규 모듈은 DOM/API/media/clipboard 의존성을 주입받아 fake로 검증한다. 파일 탭은 정규화 catalog path로 재사용하고 닫힌/세대가 바뀐 요청을 폐기한다. 녹음은 preflight→media→stop/track cleanup→audio-only 전사1회→local instruction/copy 순서며 reset/pagehide는 delayed media 결과까지 정리한다. 실제 모델·마이크·clipboard·오디오 서비스 테스트 호출은 없다.

### VW-N02 — 신규 독립 모듈 첫 검증 계획

- `node --test tests/viewer-workspace.test.mjs`로 탭 중복/역순/닫기/reset, 서버 NFD 경로/허용 링크, multipart boundary, key 없는 media0, 늦은 권한 반환 track cleanup, 파일 전환/선택 시간 기록, audio-only1POST, 용량 초과/늦은 전사 폐기와 지시문 형식을 확인한다. 모든 media/API는 주입한 대역이고 clipboard는 아직 호출하지 않는다.
- 먼저 노출되는 API multipart와 모듈 수명 결함을 보존하고 같은 표적을 재검증한다. 현재 `createApi`는 모든 body를 JSON으로 만들기 때문에 FormData 사례는 수정 전에 실패해야 한다.

- VW-N02 첫 결과: **8 pass / 1 fail**, exit1, 약129ms. microphone/key/reset/timeline/tab 경계는 통과했으며 실제 FormData가 `'{}'` JSON으로 바뀌는 반례가 실패했다. FormData는 Content-Type을 브라우저가 정하도록 두고 body를 그대로 전송하도록 보정한 뒤 동일 모듈 표적은 **9 pass**, exit0, 약125ms였다. 실제 microphone/clipboard/provider 호출은 없다.
- root/독립 reviewer가 async renderer import 중 같은 파일 refresh 및 Mermaid 대기 중 탭 전환의 concrete race를 확인했다. cached node/file identity를 capture해 늦은 import/catch를 폐기하고, 비활성 preview는 hidden 상태로 DOM 연결을 유지한다. 미완료 render state는 재활성화 때 재시작 가능하고 ready는 재사용한다. 닫기/reset은 node/body를 지우며 pending scroll restore는 epoch/path로 차단한다. 이 두 race의 독립 browser 소유는 다른 담당자다.
- recorder instruction은 trusted `heading.dataset.documentAnchor`를 사용해 실제 duplicate heading suffix까지 보존하고 selection과 시각을 함께 출력한다. raw model data 속성은 기존 sanitizer가 제거한다. root가 이 trusted DOM 속성만 renderer에서 추가했다.

### VW-N03 — App 연결과 기존 Node 소비자

- 파일/녹음 controller는 render 밖 인스턴스로 유지해 settings/modal 및 문서 탭 전환이 media를 중지하지 않게 연결했다. 파일 탭 중에는 chat DOM을 갱신하지 않고 복귀 때 최신 state를 표시한다. 기존/새 대화 선택은 chat 탭으로 돌리고, 명시 파일 저장 완료 때만 tree를 한 번 갱신한다. key 설정은 별도 write-only form이며 녹음 사건은 전사 서버로 보내지 않는다.
- 중간 조립 `node --test tests/web.test.mjs tests/viewer-workspace.test.mjs tests/design-package-ui.test.mjs`는 **35 pass / 20 fail**, exit1, 약8.81초였다. 실패는 기존 Node Element.closest 대역이 selector와 무관하게 자기 자신을 반환하는데 넓어진 문서 guard가 truthy node만 보고 모든 button을 문서로 취급한 것에 모였다. 실제 guard를 trusted document/file preview dataset 존재 검사로 명확히 하여 보호를 유지하고 대역도 기존대로 둔다. assertion을 지우지 않는다.
- 다음 같은55개 소비자를 다시 실행한다. 구문 검사와 결과를 확인한 뒤 fake-media browser 표적 및 독립 file race 검증으로 진행한다. 실제 clipboard/mic/OpenAI 요청은 하지 않는다.

- VW-N03 동일 소비자 재검증은 **55 pass**, exit0, 약617ms였다. plain text의 반복 문장 위치를 DOM Range offset→line/endline으로 보존하는 반례를 더한 신규 모듈 표적은 **10 pass**, exit0, 약153ms였다. root의 최종 전체 Node는 **60 pass / 1 opt-in skip**, exit0이다. 모든 동작 대역은 주입했으며 실제 microphone/clipboard/provider 호출은 없다.

### VW-B03 — 실제 패널 geometry와 짧은 저장 화면 계획

- root가 첫 voice browser에서 sidebar 접기 뒤 main 폭0을 desktop/mobile에서 발견했다. CSS grid 자동 배치가 display:none인 첫 sidebar를 건너뛰지 않아 main을0px track에 넣은 제품 결함이었다. `.sidebar/.workspace/.aux-panel`에 명시 grid-column/row를 부여했다. root의 initial voice6 중4 pass/2 fail 및 독립 race의 initial2 pass/2 fail은 각 담당 이력에서 보존한다. 강제 click이나 visibility 완화로 감추지 않는다.
- root 공유 고정 binary `target/architalk-viewer-fixed`, version0.4.0, SHA256 `427323fe65e4806639e2fce9a1105b0e238958717fe926ac623766bbd24a4a71`를 사용한다. 이 바이너리는 명시 grid 수정·plain-text line 기록 및 source freeze를 포함한다. 이후 제품 source 추가 변경은 없다.
- 실행 전 명령: `ARCHITALK_BROWSER_BINARY=target/architalk-viewer-fixed ARCHITALK_BROWSER_RUN=viewer-workspace-b03-20261118 npx playwright test tests/browser/viewer-workspace.spec.mjs` (desktop/mobile 각2case, 총4). 먼저 `node --check tests/browser/viewer-workspace.spec.mjs`를 확인한다.
- 첫 소비자는 임시 Markdown/link/plain-text 파일만 읽고 파일 중복탭/close/chat, workspace tab keyboard scope, 실제 좌우·양쪽 collapse geometry, tree context path/name copy(clipboard stub)와 수동 fallback을 확인한다. 외부 요청/pageerror/AI쓰기0을 요구한다. 두 번째는 copied fake CLI로만 합성10문서 생성 후 default source접힘,360px 높이에서 footer 접근·본문 scroll/preview 확장, 실제10파일 저장과 명시 tree refresh를 확인한다. 사용자 프로젝트/실제 모델/마이크/clipboard/API 호출은 없다.
- 독립 file render race 담당은 같은 fixed binary에서 desktop/mobile **4 pass**, exit0, 약4.9초를 통지했다. 지연 import+refresh/비활성 Mermaid 왕복·동일 ready node·main 폭>200·GET-only·pageerror/외부0이었다. 해당 소스와 spec은 이 담당자가 편집하지 않는다.

- VW-B03 최초 구문 검사/runner는 신규 spec의 NFC 파일명 검사에 닫는 괄호1개가 많아 **SyntaxError / no tests found**, exit1이었다. 제품 browser는 실행되지 않았다. 괄호를 바로잡고 구문 검사 성공 뒤 같은4case를 `viewer-workspace-b04-20261118`로 실행한다. 이 실패를 성공 결과와 구분한다.

- VW-B04 실제4case 결과는 **0 pass / 4 fail**, exit1이었다. 두 viewport 모두 좌우/양쪽 geometry·파일 rendering/link focus·중복 read1과360px footer/저장10파일/tree11까지 통과한 뒤, 테스트가 OS/server 경로를 추측한 두 비교에서 실패했다. 절대 경로는 macOS `/var` symlink를 canonical화한 `/private/var`가 올바른 서버 값이고, 저장은 문서별 root 바로 아래가 아니라 서버가 생성한 natural package 하위 폴더다. 제품 경로 동작은 변경하지 않는다.
- VW-B05 계획: 절대 copy 비교는 `fs.realpath`의 실제 canonical 값, 저장 tree 비교는 `exported.result.paths` 전체10경로를 NFC 동등 비교한다. 나머지 geometry/접근/부작용 판정은 그대로 둔다. 구문 검사 후 같은4case를 `viewer-workspace-b05-20261118` run으로 실행한다.

- VW-B05 최종 결과: **4 pass**, exit0, **14.2초**. desktop/mobile 파일 tree/tabs/cross-file anchor/context menu와360px높이 저장 소비자가 통과했다. canonical path oracle만 보정했고 제품 binary/source는 VW-B03 고정 SHA 그대로다. 자동복사/수동fallback은 stub clipboard, 모델은 temp copied CLI,10개 결과파일은 temp project에만 저장되었다.
- 실제 geometry는 main 폭>200·viewport 내rightedge·document horizontaloverflow0, desktop left/right닫기의 폭 증가 각각>180/>250을 판정했다. 좌우 restore controls는 항상 접근 가능했다. preview HTML은 text node로 표시되었고 내부 문서 링크는 앱 tab+trusted heading focus로 끝났다. 중복 README read1, filetabs Home/End가 auxpanel 선택을 바꾸지 않음, close와새대화 chat복귀, 탐색 과정 API non-GET0/session0을 확인했다.
- 저장은10개 원문 details 기본closed, footer/action이360px viewport 내 유지됨, 원문1개 확장/본문 최하단 scroll 뒤에도 접근 가능함, 실제10파일 저장·tree11로 갱신됨을 확인했다. tree의10경로는 실제 export.result.paths 전부와 NFC 동등 비교했다. user message1/design logicalrun1이며 자동 AI 재생성은 없다.
- 최종 PNG를 직접 열어 시각 확인했다: [mobile360px 저장](../../target/browser-artifacts/viewer-workspace-b05-20261118/viewer-workspace-ten-docum-fd748-reachable-on-a-short-screen-mobile/ten-files-short-screen.png), [desktop360px 저장](../../target/browser-artifacts/viewer-workspace-b05-20261118/viewer-workspace-ten-docum-fd748-reachable-on-a-short-screen-desktop/ten-files-short-screen.png), [mobile tabs/chat복귀](../../target/browser-artifacts/viewer-workspace-b05-20261118/viewer-workspace-file-tree-023ab-g-stay-inside-the-workspace-mobile/file-tabs-panels.png). 두 저장 화면 모두 본문과 독립된 footer가 완전히 보이며 가로 잘림이 없다. 이 PNG들은 generated artifact로 git에는 추가하지 않는다.
- `node --check` 신규browser spec 및 app/file-viewer/document-recorder/viewer-menu 각모듈, `git diff --check` 모두 exit0이었다. 이 담당자의 제품 source와 test/history는 최종 동결한다. Root는 별도 fake-media8/기존browser전체 최종 소비자 결과를 통합 기록하며 이 담당자는 중복 실행하지 않는다.
