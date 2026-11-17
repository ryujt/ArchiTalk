# 대화 완료 뒤 최신 답변 스크롤 브라우저 검증

## 사전 범위 — 2026-11-18 08:24 KST

- 기준: `main@5954230`. 기존 `src/ai.rs` Codex 진행 안내 한국어 문구 변경은 이번 스크롤 재현과 무관하다. 제품 스크롤 수정 전 상태를 먼저 검증한다.
- 목표: 긴 대화가 viewport보다 커진 실제 desktop/mobile 화면에서 매 턴 완료 후 최신 답변과 다음 입력창을 볼 수 있고, 과거 내용을 읽는 동안 단순 snapshot 갱신이 위치를 빼앗지 않는지 확인한다.
- 소유 파일: `tests/browser/chat-scroll.spec.mjs`, 이 기록. 공용 fixture·제품 파일은 읽기만 하며 색인 병합은 root 담당자에게 전달한다.
- 이전 기록: [implementation-web-browser](implementation-web-browser.md) B04의 오래된 snapshot 경합 수정과 B05 workflow 검증, [model-choices-browser-20261118](model-choices-browser-20261118.md) B01의 desktop/mobile 설정·워크플로 4건 통과를 확인했다. 기존 여정은 실제 스크롤 위치를 검사하지 않았으므로 이번 반례로 그 빈틈을 검증한다.
- 환경: 격리된 임시 프로젝트·설정·loopback HTTP/SSE·실제 Rust/SQLite·복사된 기존 Claude workflow fixture subprocess. 저장소가 새로 시작하는 headless Chrome desktop 1365×900, mobile 390×844. 실제 모델·사용자 브라우저·외부 요청은 사용하지 않는다.

| 시나리오 | 실제 관찰·판정 |
| --- | --- |
| 여러 긴 대화 턴 완료 | 긴 합성 user 입력 3턴, 실제 fixture 완료 및 chat-turn 2/4/6개 확인. 각 완료 뒤 자동 스크롤을 돕는 테스트 조작 없이 마지막 답변 하단과 answer-form이 viewport에 있는지 측정 |
| 긴 대화 전제 | thread 높이가 viewport보다 크다는 실제 layout 측정 |
| 과거 기록 읽기 | 앞선 user turn을 스크롤해 읽는 위치에서 브라우저 online 이벤트로 실제 서버 snapshot 재조회. 읽는 turn의 viewport 위치 보존과 최신 답변으로 강제 이동하지 않음 |
| 기존 초안 이후 대화 | 실제 fixture design/critic로 초안 생성·review 진입, 대화로 돌아가 긴 추가 답변 제출. 다음 conversation 완료 시 4개 chat-turn과 최신 답변/입력창 노출, 기존 초안 보존 |
| 작성 중 화면 유지 / 다음 단계 | 테스트별 임시 executable의 기존 SLOW_REQUEST 지연만 30→2초로 변경해 실제 subprocess 관찰 시간을 확보. 기존 user/assistant 기록+작성중 bubble+중지버튼+composer 유지, 완료 후 bubble/중지 제거와 primary 다음 단계 CTA/최신 답변/입력창 viewport 확인 |
| 넓은 화면 | 첫 desktop 시나리오를 1920×900으로 확장하고 긴 user 입력을 36줄로 늘림. main의 실제 좌우 padding을 뺀 가용폭을 content/composer가 2px 이내로 채움, content >1000px, 가로 넘침 없음 및 viewport screenshot |
| 증거·격리 | 턴별 viewport screenshot/geometry, pageerror·외부 요청·Bearer 위반 0건. fixture·서버·context 소유 자원만 정리 |

수정 전 계획: `ARCHITALK_BROWSER_RUN=<고유 ID> npm run test:browser -- tests/browser/chat-scroll.spec.mjs`. 수정 후에는 준비 통지를 받아 신규 및 기존 browser suite를 실행한다. 실패·재검증은 별도 항목에 누적한다.

## 실행 기록

### S01 — 새 테스트 구문

- 시각: 2026-11-18 **08:24:11 KST**에 종료 관찰.
- 명령: `node --check tests/browser/chat-scroll.spec.mjs`.
- 결과: exit 0.

### B01 — 수정 전 실제 브라우저 반례

- 시작: 2026-11-18 **08:24:11 KST** 직후. 종료 출력 관찰: **08:24:47 KST**. run ID: `run-1794957840000`.
- 명령: `ARCHITALK_BROWSER_RUN=run-1794957840000 npm run test:browser -- tests/browser/chat-scroll.spec.mjs`.
- 기대: desktop/mobile에서 실제 완료된 긴 대화의 마지막 답변이 viewport 밖에 남는 사용자 버그를 확인한다. 실행되지 않은 결과를 통과로 기재하지 않는다.
- 결과: **desktop/mobile 2건 모두 실패**, exit 1. 제품 build **2.53초**. 이 첫 실행에는 긴 3턴+과거 읽기 snapshot 보존 시나리오만 있었다. 실행 이후 추가한 초안 존재 분기는 B01에서 실행했다고 주장하지 않는다.
- 실제 반례: 두 viewport 모두 실제 fixture conversation 3건 완료, chat-turn 2/4/6개 표시, thread 높이 > viewport를 확인했다. **desktop 첫 턴의 최신 답변/입력창이 둘 다 viewport 밖**, desktop 후속 2/3턴은 보임. **mobile은 1/2/3턴 모두 최신 답변/입력창이 viewport 밖**이었다. UI 재생성 후 최신 응답을 보여주지 않는 제품 결함으로 분류했다. 과거 user turn을 읽는 위치에서 실제 snapshot을 다시 읽었을 때 viewport 위치는 2px 이내로 보존됐다.
- 증거: `target/browser-artifacts/run-1794957840000/`의 desktop/mobile 각 `01/02/03-completed-turn.png`, `04-reading-after-snapshot.png`, 실패 screenshot/context. viewport screenshot은 테스트가 완료 후 자동 스크롤을 돕기 전에 찍었다. 첫 실행의 geometry attachment는 line reporter에 별도 파일로 보존되지 않아 후속 실행부터 JSON 파일도 명시적으로 저장하도록 보강했다.
- 후속: 제품 담당자에게 baseline build 완료와 실제 실패를 전달했다. 제품 수정 후 같은 위치 assertion을 재검증한다. 비밀·외부 요청·pageerror 위반은 발생하지 않았다.
- 시각 확인: B01 뒤 desktop의 `01-completed-turn.png`, mobile의 `03-completed-turn.png`를 실제로 열었다. 완료 상태인데 긴 이전 user 텍스트가 viewport를 차지하고 최신 assistant/입력창은 보이지 않는 결과를 확인했다.

### S02 / B02 — 수정 후 회귀 준비

- 준비: 2026-11-18 08:25~08:27 KST. 추가 범위는 root/Critic가 확인한 기존 초안 뒤 대화 완료 분기 및 사용자가 이어서 요청한 primary 다음 단계 CTA/작성 중 인라인 표시·넓은 화면 가용폭 사용이다. baseline 실패 assertion을 약화하지 않았다. 임시 fixture 지연 변경은 root가 공용 fixture에 추가한 `server.configureSlowFixture(2)` 소유 API를 사용한다. 테스트가 임시 경로를 추론하거나 공용 실행 파일을 변경하지 않는다.
- 명령: `node --check tests/browser/chat-scroll.spec.mjs` 후 `ARCHITALK_BROWSER_RUN=run-1794958500000 npm run test:browser`. 신규 2개 시나리오×desktop/mobile 4건 + 기존 설정/workspace 4건으로 예상 8건이다.
- 시각: **2026-11-18 08:27:26 KST**에 구문 검사 exit 0 확인 후 browser 명령 시작, 종료 출력 관찰 **08:27:53 KST**.
- 결과: **8건 통과·0건 실패**, exit 0. 제품 build **2.75초**, Playwright **22.8초**. B01 실패 이후 제품이 수정된 첫 전체 회귀이며 재시도는 없었다.
- 신규 4건에서 실제 확인: 긴 3턴 모두 완료 직후 최신 assistant 하단·composer·primary 다음 단계 CTA가 viewport에 들어옴. 같은 HTTP/SSE/fixture 경로의 3번째 요청을 2초 지연하는 동안 기존 user/assistant 내용과 composer가 유지되고 `답변 작성 중…` bubble/중지버튼이 표시됨. 완료 후 pending/중지 제거. 과거 내용을 읽는 동안 real snapshot 재조회는 해당 turn의 viewport y를 2px 이내로 보존. fixture design/critic로 만든 기존 초안 이후 conversation도 최신 chat 4개와 입력창/초안 다시 정리 CTA로 돌아오고 초안 1개 보존.
- 넓은 화면 수치: desktop viewport/document **1920/1920px**, main padding 제외 가용폭/content/composer 모두 **1230px**. 실제 thread 높이는 매 턴 viewport 900px를 넘었고 desktop 마지막 assistant 하단은 약 **559px**, composer는 약 **731~850px**에 있었다. mobile도 모든 완료 턴에서 최신 답변/입력창/CTA viewport와 가로 넘침 없음이 통과했다.
- 기존 4건에서도 설정 목록/직접 입력·저장·reload·next Run, 초안 실제 파일 저장·복구·모달 초점·모바일/tablet 흐름이 유지됐다. pageerror·외부 요청·Bearer 위반 0건. Node의 NO_COLOR/FORCE_COLOR 환경 경고 외 실패 출력은 없었다.
- 산출물: `target/browser-artifacts/run-1794958500000/`. 새 시나리오 viewport PNG 13장, 기존 설정/workspace PNG 17장 및 geometry JSON. 토큰·헤더·trace/HAR/video는 기록하지 않았고 기존 7일 artifact retention을 따른다.

### R01 — 실제 화면과 변경 diff 확인

- 시각: 2026-11-18 08:28 KST, B02 종료 후.
- 실제 열어 본 수정 후 PNG 4장: 긴 desktop 대화 `05-wide-conversation.png`, mobile `00-inline-pending.png`, `03-completed-turn.png`, 기존 초안 뒤 mobile `01-conversation-after-draft.png`. 넓은 중앙 작업 영역, pending 시 이전 텍스트/중지/입력창 유지, 완료 시 새 답변/입력창/명확한 다음 단계 버튼, 기존 초안 안내 줄바꿈을 확인했다.
- 폴더: `chat-scroll-completed-long-61bd3-rves-older-reading-position-{desktop,mobile}/`, `chat-scroll-a-conversation-e00a9--new-reply-and-its-composer-{desktop,mobile}/`.
- 제품/공용 fixture diff를 읽기 전용으로 확인했다. 최신 대화 변화/장면 진입 시에만 composer 쪽을 따라가고 동일 snapshot은 읽는 위치를 복원하며, 초안 이후 conversation 완료는 interview를 표시하고 design/critic 완료는 기존 review를 유지하는 경계를 대조했다. fixture helper가 captured 임시 executable만 바꾸는 점도 확인했다. 요청 범위에서 필수 잔존 결함은 발견하지 못했다.

### S03 / B03 — SmartMD 저장 경계 반영 후 최종 브라우저 회귀

- 준비: root에게 형식 Developer 완료/동결 및 새 대표 fixture 동반 복사 wiring 통지를 받은 뒤 시작한다. B02는 UI 수정 당시의 기존 Markdown fixture 증거로 보존한다. 이번 반복 근거는 내장 smartmd-v1 생성 계약·구조 검증·초안 저장 거절/실제 저장물 형식이 바뀌었다는 점이다.
- 쓰기 추가 허용: root가 `tests/browser/workspace.spec.mjs`의 실제 저장 bytes assertion 소유권을 이양했다. 기존 흐름은 유지하고, 디스크 파일 첫 줄 frontmatter·`style: smartmd`·4종 전용 DSL fence·info callout·SmartMD 표 사양/GFM 표가 남는 검사만 추가했다.
- 명령: `node --check tests/browser/workspace.spec.mjs` 후 `ARCHITALK_BROWSER_RUN=run-1794959400000 npm run test:browser`.
- 범위: 신규 대화 scroll/작성중/초안후대화 4건 + 기존 설정/workspace 4건, 실제 HTTP/SSE/SQLite/파일과 새 SmartMD fixture subprocess. 실제 모델 호출은 없다. 상세 renderer API 호환성은 [smartmd-renderer-20261118](smartmd-renderer-20261118.md)의 독립 Node 증거이며 이번 ArchiTalk 화면은 실제 Mermaid/SVG viewer 시각 검증을 대체하지 않는다.
- 시각: **2026-11-18 08:37:22 KST** 구문 검사 exit 0 확인 후 browser 명령 시작, 종료 출력 관찰 **08:37:48 KST**.
- 결과: **8건 통과·0건 실패**, exit 0. cargo build **2.81초**, Playwright **23.2초**. 한 번 실행했으며 재시도·부분 재실행은 없었다.
- 실제 저장 결과: desktop/mobile 각각 새 대표 SmartMD가 fixture subprocess→형식 계약→design/critic→초안 검토→preview→apply→completed export를 지나 디스크에 저장됐다. 저장 파일이 첫 byte부터 `---\n` frontmatter로 시작하고 `style: smartmd`, navigation/jobflow/state/layout 전용 fence, info callout, SmartMD 표 사양 바로 뒤 GFM 표를 유지함을 실제 bytes로 확인했다. 기존 `내용 상태: draft` 표시도 남았다.
- 영향 회귀: 3턴 최신 답변/입력창/CTA viewport·과거 snapshot 읽기 위치·실제 pending 표시·기존 초안 뒤 대화·wide 가용폭·설정 목록/직접 입력/다음 Run·기존 Run 불변·복구·모달 초점/mobile/tablet 검사도 통과했다. pageerror·외부 요청·Bearer 위반 0건이다.
- 산출물: `target/browser-artifacts/run-1794959400000/`의 PNG **30장**과 geometry JSON. 기존 artifact 보존/비밀 비기록 경계를 유지했다.

### R02 — B03 최종 산출물 시각 확인

- 완료 관찰: **2026-11-18 08:38:25 KST**. 실제로 연 PNG는 B03 desktop workspace `02-review.png`, mobile workspace `03-saved.png`, mobile 긴 대화 `03-completed-turn.png` 총 3장이다.
- 새 SmartMD 원문이 초안 검토 영역에 줄바꿈되어 표시되고 긴 문서가 가로 넘침을 만들지 않으며, 모바일 저장 완료 경로/동작 버튼과 대화 완료 뒤 최신 답변/입력창/CTA가 보이는 것을 확인했다.
- 현재 ArchiTalk 검토 영역은 SmartMD **원문 텍스트**를 보여 준다. 이 PNG를 mdview의 callout/table/DSL 시각 렌더 결과라고 해석하지 않는다. 실제 mdview 변환 API 호환성은 별도 [smartmd-renderer-20261118](smartmd-renderer-20261118.md) 기록에서 확인했다.
- 이번 변경 범위의 필수 잔존 브라우저 결함은 발견하지 못했다.

## 한계와 후속

- 실제 모델·계정 권한·응답 품질·실제 모바일 OS/IME·다른 브라우저는 검증하지 않았다. 모델 subprocess만 기존 명시적 fixture다. 앱의 HTTP/SSE·설정·상태·브라우저 layout은 실제 경로다.
- 작성 중 중지버튼의 노출을 이번 새 시나리오에서 확인했다. 취소 subprocess 종료 자체의 전수 재검증은 이번 스크롤 요청 범위에서 하지 않았다. 기존 취소 계약 테스트를 대체하지 않는다.
- B01 실패 증거는 보존했다. B02 이후 바뀐 SmartMD 저장 경계의 B03 최종 회귀도 통과했으며 미종결 브라우저 실행은 없다.
