# 실제 웹 워크스페이스 구현 검증

기준: `main` / `1052929`에서 실제 Rust 제품을 작성 중인 작업 트리. 소유 범위는 `web/index.html`, `web/app.js`, `web/style.css`, `tests/web.test.mjs`다. 설계 모형 `docs/design/20261117-190055/to-be/details/screens/workspace.html`은 변경하지 않았다. 시각은 실제 관측한 KST이며 측정하지 않은 시각을 추정하지 않는다.

이전 이력: `ui-20261117-static.md`의 UI-S01/S02/T01과 `20261117-tobe-validation.md`의 R01~R04를 먼저 읽었다. 이전 결과는 설계 모형의 정적 검사이고 실제 제품이나 브라우저 통과의 근거로 사용하지 않았다. 초안 미생성/과거 버전, 모름의 보존, 무료 CLI 진단과 로그인/모델 확인의 분리, no-clobber 저장, 종료 미확인 시 새 실행 차단을 검증 기준으로 이어받았다.

## WEB-01 — 제품 경계의 단위·상태 검사

- 상태: 통과, 대역을 사용한 제품 규칙 검증. 첫 13건 및 보강 후 16건 실행의 개별 벽시계 시작·종료는 미측정이다. 모두 WEB-02의 측정 시작 전 실행했다. 가장 최근 보강 실행은 Node가 16건 통과, 0건 실패, 약 304ms로 보고했다.
- 가정: capability는 fragment에서 즉시 지우고 API/SSE의 Authorization header로만 전송한다. UTF-8/CRLF 분할과 불완전 SSE frame이 최종 상태를 잘못 만들지 않는다. 취소 접수·저장 POST 접수·스트림 단절을 완료로 간주하지 않는다. 현재 설계 기록과 오래된 초안을 분리한다. 설정은 신뢰된 executable/profile을 변경하지 않는다.
- 명령: `node --check web/app.js`, `node --test tests/web.test.mjs`.
- 제품 경로: 테스트는 `web/app.js`를 그대로 불러온다. API client/SSE parser/settings 변환/초안 gate의 공개 함수와 `startWorkspace`를 사용한다. 별도 제품 구현을 복제하지 않는다.
- 범위: fragment 제거 순서·storage 실패의 메모리 fallback, 모든 fetch의 Bearer/same-origin/no-redirect/no-store, HTTP 오류와 stream MIME, SSE 모든 byte 경계·한글/emoji·BOM·CRLF·다중 data·NUL ID·미완성 frame·buffer 상한·abort, design_revision과 session revision 분리, trusted 설정 보존, 상대 경로 검증, AI/프로젝트 HTML escape, 미생성 문서, 실제 서버 snapshot만 표시, UUID/revision command envelope, 서버 확인 전 취소 완료 금지, 과거 초안 본문 보존/승인 금지/초안 저장 허용, export conflict/failed의 성공 표시 금지, durable cursor 후 snapshot 재동기화, 연결 단절 후 읽기 전용 fallback과 중복 run 없음, termination unknown의 새 실행 차단, apply 응답 유실 후 정확한 export GET 확인.
- 증거 수준: fetch 대역과 작은 DOM 대역으로 제품 상태·프로토콜·안전한 마크업을 검사했다. 실제 브라우저 렌더링/스크린리더/포커스와 실제 CLI·HTTP 서버 전 구간의 통과를 뜻하지 않는다.

## WEB-02 — 측정된 최종 복합 확인과 공백 실패

- 시작: **2026-11-17 21:07:48.060580 KST**. 종료 벽시계는 assertion 실패로 출력되지 않았다. 도구가 반환한 전체 실행 시간은 약 0.458초다.
- 명령: Python wrapper에서 `node --check web/app.js`, `node --test tests/web.test.mjs`를 순차 실행한 뒤 Python `HTMLParser`와 줄 끝 공백 검사를 수행했다.
- 실제 결과: Node 구문 검사 exit 0. 제품 테스트 **16건 통과, 0건 실패**, Node 보고 실행 시간 약 304ms. HTML 중복 ID/inline handler 검사는 오류 없이 통과했다. 마지막 공백 검사에서 **`web/style.css trailing whitespace`** assertion이 발생해 wrapper는 exit 1이었다.
- 수정: 설계 모형에서 재사용한 CSS의 줄 끝 공백을 제거했다. 제품 JS·계약·테스트 판정은 변경하지 않았다.
- 보존: 복합 실행의 실패를 제품 테스트 통과로 덮어쓰지 않는다. 아래 WEB-03이 해당 정적 결함의 재검증이다.

## WEB-03 — 정적 결함 수정 재검증

- 상태: 통과.
- 시각: **2026-11-17 21:07:58.624443~21:07:58.625262 KST**.
- 명령: Python `HTMLParser`로 `web/index.html`의 ID/자산/inline handler를 확인하고 제품 웹 3파일과 `tests/web.test.mjs`의 줄 끝 공백·최종 newline을 검사했다.
- 실제 결과: exit 0. HTML의 **20개 ID 중복 없음**, 자산이 동일 origin의 `/style.css`, `/app.js`만 사용함, inline script/handler 없음, 4파일의 줄 끝 공백/최종 newline 통과.
- 테스트 범위 선택: 공백만 수정했으므로 WEB-02에서 통과한 16건을 다시 전체 반복하지 않았다.

## WEB-04 — 자체 검토 후 상태·접근성 보완

- 상태: 통과. WEB-03 이후 실행했으며 개별 명령의 벽시계는 미측정이다. 도구는 마지막 명령 약 0.401초, Node 테스트 약 318ms로 보고했다.
- 변경: 백그라운드 재연결 후 연결 안내를 즉시 갱신한다. 모달에서 돌아올 때 현재 DOM에서 원래 동작을 다시 찾아 초점을 돌리고, 대상이 사라졌으면 main으로 복귀한다. 과거 초안의 저장 상태를 현재 세션의 승인 상태로 표시하던 문제를 고쳐 선택한 Draft.content_status를 사용한다.
- 명령: `node --check web/app.js && node --test tests/web.test.mjs`.
- 결과: exit 0, 구문 유효, 기존 **16건 통과·0건 실패**. 이 상태로 UI 쓰기를 동결하고 통합 담당에게 넘겼다. native dialog의 실제 브라우저 포커스 동작은 여전히 미검증이다.

## WEB-05 — 독립 검토에서 발견한 세션 없는 진단 복구 보완

- 상태: 통과. 통합 담당이 수용한 Critic 지적 3번의 UI 범위를 다시 열어 수정했다. backend 파일은 직접 수정하지 않았다.
- 시각: **2026-11-17 21:13:13.980361~21:13:14.522888 KST**.
- 문제: 첫 세션 생성 전 help 진단의 종료가 미확인이면 전역 실행 gate는 잠기지만, 세션이 없어서 브라우저에 수동 종료 확인 수단이 없었다. 진단 응답의 application error도 HTTP 200에 들어오면 표시되지 않았다.
- 변경: 세션 없는 전역 diagnostic run의 진행·종료 미확인 장면을 제공한다. 종료를 사용자가 직접 확인하면 해당 run의 recover에 workspace_revision을 전송한다. 세션이 있는 diagnostic은 해당 세션 revision을 조회한다. 진단 실행 중에는 새 AI 작업을 막고 snapshot만 주기적으로 조회하며 AI를 자동 실행하지 않는다. diagnostics의 오류 응답을 표시하고 성공/실패 모두 workspace를 재조회한다.
- 명령: 측정용 Python wrapper에서 `node --check web/app.js`, `node --test tests/web.test.mjs` 실행.
- 결과: 두 명령 exit 0, **19건 통과·0건 실패**, Node 보고 약 349ms. 추가 3건은 세션 없는 unknown 진단→workspace revision 수동 복구, 실행 중 진단의 신규 AI 차단/읽기 전용 polling, HTTP 200 application error 표시와 전역 gate 재조회다. 기존 16건도 통과했다.
- 후속 소수정: diagnostic 장면에서 일반 ‘다른 대화’ 안내가 중복되지 않도록 분기 조건을 좁혔다. 같은 두 Node 명령을 다시 실행해 exit 0, 19건 통과를 확인했다(Node 약 338ms, 개별 벽시계 미측정).
- 한계: 제품 모듈을 호출하는 fetch/DOM 대역 검사다. 실제 진단 프로세스와의 공유 slot, OS 종료 증거는 backend/AI 통합 검증 대상이며 이 테스트로 대신하지 않는다.

## 미검증과 구현 범위

- 브라우저 시각 QA, 실제 desktop/mobile 폭, native dialog의 포커스 가두기·복귀, 실제 IME/스크린리더는 통합 담당의 브라우저 검증 대상이다. 정적 DOM 대역으로 통과를 대체하지 않는다.
- 이 기록의 16건은 외부 CLI 동작의 증거가 아니다. 실제 서버/실제 또는 fixture CLI/파일 저장·재시작 결과는 통합 및 AI·파일 담당 이력에서 별도로 판정한다.
- Markdown은 안전한 plain text로 표시한다. raw HTML 실행·외부 이미지·범용 문서 편집기를 제공하지 않는다.
- 실행파일과 Codex profile은 읽기 전용이며 사용자 전역 설정에서 변경한다. 모델/effort는 직접 입력하고 호환성·로그인은 확인된 범위만 표시한다. 다음 실행부터 적용되며 자동 재시도하지 않는다.
- 저장은 서버 미리보기와 `completed` export 결과를 받은 뒤에만 완료로 표시한다. 충돌/실패는 기존 결과를 성공으로 바꾸지 않고 새 미리보기로 돌아간다. 서버의 파일별 판정·외부 수정 내용의 시각적 diff는 별도 범위다.
