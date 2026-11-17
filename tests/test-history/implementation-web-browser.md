# 독립 headless 제품 브라우저 검증

이 파일은 아래 실행들을 마친 뒤 실제 도구 출력과 보존된 artifact를 근거로 **사후 정리**했다. 실행 전에 작성된 기록으로 주장하지 않는다. 개별 시각을 측정하지 않은 실행은 그대로 표시한다.

기준: `main` / `1052929`에서 실제 제품 구현 중인 작업 트리. 이전 `implementation-web.md` WEB-01~05의 19건은 fetch/DOM 대역 검사였고, `ui-20261117-static.md`와 `20261117-tobe-validation.md`의 브라우저 QA는 host 부재로 미실행이었다. 이번에는 저장소가 직접 실행하는 독립 브라우저로 그 미검증 경계를 확인했다. 기존 사용자 브라우저·프로필·탭·CDP에는 연결하지 않았다.

| 항목 | 범위와 판정 기준 |
| --- | --- |
| 목표 | 실제 HTTP 화면에서 시작→질문→design/critic→초안 검토→파일 저장→모델/effort 설정→다음 실행→새로고침 복구 |
| 제품 경계 | Rust CLI/HTTP, 실제 `web/*`, 실제 SSE, SQLite 상태, subprocess adapter, no-clobber 출력 파일 |
| 외부 대역 | 모델 CLI만 `tests/fixtures/claude-workflow.py`의 명시적 fixture. 원격 모델 호출 0건 |
| 필수 확인 | 서버 완료 상태와 실제 저장 파일 bytes, 승인 조건 미충족 시 승인 차단, next-run 설정, 재실행 없는 복구, 동일 origin/Bearer, HTML 비실행 |
| 시각·접근성 | desktop 1365×900, mobile 390×844, tablet 768×1024의 가로 넘침·모달 읽기·Tab/Shift+Tab·배경 inert·Escape/초점 복귀 |
| 격리 | 테스트별 새 임시 프로젝트/전역 설정/fixture, 임의 loopback 포트, 새 브라우저 context. 테스트가 시작한 child/context만 정리 |

## B01 — 도구 준비와 정확한 실행 환경

- 공식 npm registry에서 조회한 `@playwright/test` **1.65.1**을 exact dev dependency와 lockfile로 고정했다. `npm install --ignore-scripts --registry=https://registry.npmjs.org`는 3패키지 설치, audit 0 vulnerabilities로 완료됐다.
- 환경: Node **v26.12.1**, npm **11.20.0**, macOS의 독립 headless Google Chrome **157.0.8103.79**. Playwright `channel: chrome`으로 새 프로세스를 실행했다. 기존 Chrome 세션을 제어한 결과가 아니다.
- 제품 화면을 담은 `include_str!`가 최신이어야 하므로 `npm run test:browser`의 pre script가 매번 `cargo build --bin architalk`를 수행한다.
- 구현 근거: [Playwright use 설정](https://playwright.dev/docs/test-use-options), [공식 브라우저 설정·설치](https://playwright.dev/docs/browsers). 본 테스트에서는 설치된 Chrome을 사용했고 Chromium 다운로드는 실행하지 않았다.
- 재현 명령: `npm ci`, `npm run test:web`, `npm run test:browser`. Chrome이 없는 환경은 먼저 `npx playwright install chromium` 후 `ARCHITALK_BROWSER_CHANNEL=chromium npm run test:browser`로 실행할 수 있다. 이 대체 경로의 브라우저 실행은 이번 환경에서 미실행이다.
- 비밀 보호: 최초 페이지 이동에는 공개 origin만 사용한다. init script가 메모리의 capability를 실제 fragment에 넣고 제품 코드가 제거하도록 했다. reload는 제품 sessionStorage를 사용한다. trace/HAR/video/storageState를 기록하지 않으며 stdout의 전체 접속 URL도 출력하지 않는다. 화면 캡처는 합성 데이터만 담는다.

## B02 — 최초 브라우저 실행: 테스트 가정 실패

- 실행: `npm run test:browser`, run ID **run-1794918453825**. run 식별 시각은 **2026-11-17 21:27:33.825 KST**이며 개별 명령의 벽시계 시작·종료는 미측정이다.
- 결과: **desktop/mobile 2건 실패**. 두 환경 모두 실제 welcome→conversation→design/critic→초안 검토→저장 완료 및 실제 파일 확인까지 진행했다. 설정 모달에서 모든 Tab 이동 후 `activeElement`가 반드시 dialog 내부라는 assertion에 실패했다.
- 보존: `target/browser-artifacts/run-1794918453825/`의 실패 screenshot과 `error-context.md`. 토큰을 포함한 전체 URL·헤더·서버 stdout은 저장하지 않았다.

## B03 — 최소 native dialog 진단과 테스트 수정

- 독립 Chrome 새 context의 최소 HTML `<dialog>`에서도 마지막 필드 뒤 Tab이 **BODY / document.hasFocus=false**로 이동하고 다음 Tab에 dialog로 돌아왔다. background 버튼에는 초점이 가지 않았다. Chrome **157.0.8103.79**의 실제 결과를 관찰했으며 제품 가짜 DOM으로 판단하지 않았다.
- 판정: B02는 브라우저 chrome으로 이동하는 정상 경계까지 금지한 **테스트 가정 오류**였다. 제품의 포커스 코드를 바꾸지 않았다.
- 수정: 콘텐츠 초점이 dialog 안에 있거나, BODY이면서 문서 자체에 초점이 없는 경우만 허용한다. 후자는 다음 traversal에서 dialog로 돌아와야 한다. dialog가 열린 동안 header의 background 버튼에 프로그램으로 focus를 시도해도 초점이 이동하지 않는다는 검사도 추가했다. assertion 삭제나 대기시간 증가로 통과시키지 않았다.
- 재실행: **run-1794918501249**, run 식별 시각 **21:28:21.249 KST**. `npm run test:browser` **2건 통과**, Playwright 약 **7.2초**. 이 결과는 이후 발견한 B04 제품 경합의 최종 통과로 대체하지 않는다.

## B04 — 실제 브라우저가 발견한 snapshot 경합

- 시각: **2026-11-17 21:29:11.702410~21:29:32.535938 KST**.
- 명령: `npm run test:browser`. **run-1794918552269**, **desktop 1건 실패 / mobile 1건 통과**, Playwright 약 **20.3초**.
- 반례: 서버에서는 design과 critic이 완료됐고 초안도 존재했지만 desktop UI는 Interview에 남았다. `.markdown`은 15초 동안 나타나지 않았고, 화면에는 ‘현재 대화 기준 초안이 있어요’와 수동 열기만 남았다. 연결 상태는 정상으로 보였다. 실패 screenshot/context를 해당 run 폴더에 보존했다.
- 원인: 생성 직전에 시작된 같은 세션의 오래된 GET 응답이 새 command receipt를 덮을 수 있었다. 별도로 session GET은 실행 중인데 뒤이은 run GET은 완료인 혼합 snapshot도 Interview 전환을 만들 수 있었고, 후속 `keep=true`가 그 장면을 계속 보존했다.
- 최소 재현: `tests/web.test.mjs`에 지연된 생성 전 snapshot과 session/run 사이 완료 두 반례를 추가했다. **수정 전 기존 19건 통과·신규 2건 모두 실패**로 같은 Interview 잔류를 확인했다(Node 약 385ms). 테스트·실패를 지우지 않았다.
- 제품 수정: command receipt를 채택할 때 이전에 시작한 snapshot을 무효화한다. session.active_run_id가 가리키는 run이 이미 terminal이고 종료 미확인 상태도 아니라면 현재 장면을 보존하고 새 snapshot을 조회한다. 취소 응답도 같은 receipt 채택 경계를 사용한다. 임의 완료 타이머·재생성·자동 모델 재시도는 추가하지 않았다.
- 단위 재검증: `node --check web/app.js && node --test tests/web.test.mjs` exit 0, **21건 통과·0건 실패**(기존 19건 유지, 새 2건 포함, 약 384ms). 개별 Node 명령 벽시계는 미측정이며 B04 실패 이후 B05 시작 전에 실행했다.

## B05 — 수정 후 최종 실제 브라우저 검증

- 상태: 통과.
- 시각: **2026-11-17 21:31:23.271791~21:31:33.990092 KST**.
- 명령: `npm run test:browser`. 먼저 최신 제품 build 완료(약 2.81초), 이어 headless Chrome의 **desktop/mobile 2건 통과·0건 실패**, Playwright 약 **7.3초**.
- 실제 확인: fragment 제거와 sessionStorage 재사용, API/SSE Bearer와 동일 origin, HTML input 비실행, 초안 미생성에서 실제 design/critic 완료 후 문서 표시, readiness 미충족 승인 버튼 차단, preview→apply→completed export와 디스크 파일 일치, 모델/effort 저장, 기존 Run 스냅샷 불변, 다음 design Run의 설정 반영, reload→Recovery와 새 Run 없음, 두 번째 초안 복구, mobile 보조 패널과 tablet 대화 선택, 가로 넘침 없음, native modal의 background inert 및 Escape/초점 복귀. 예상하지 않은 외부 요청과 pageerror 0건.
- 캡처: **`target/browser-artifacts/run-1794918686694/`**, desktop 5장·mobile/tablet 6장, 합계 **11장**. welcome/review/saved/settings/recovery와 tablet을 담는다. 로컬 PNG 뷰어로 desktop 검토 화면, mobile 시작/복구, tablet 검토, 최종 desktop 설정과 최종 mobile 복구를 실제로 열어 배치·줄바꿈을 확인했다.
- 산출물 보존: 위 폴더와 이전 실패 run은 Git 제외인 `target/` 아래에 있다. 다음 브라우저 테스트 시작 시 **7일보다 오래된 `run-<숫자>` 폴더만** 제거한다. 별도 임시 경로의 만료/최신/무관 폴더를 만들어 retention 함수를 실행한 결과 만료된 소유 run만 삭제되고 최신 run과 무관 폴더가 보존됨을 확인했다.
- 최종 정적 확인: **21:31:59.530201 KST**, package JSON/lockfile 파싱·작성 파일 공백·artifacts Markdown의 capability URL 패턴 검사 통과. 제품/테스트 파일에 토큰 상수나 실제 사용자 데이터가 없다.

## 범위와 남은 한계

- 이번 E2E는 **실제 브라우저·실제 HTTP/SSE·실제 Rust/SQLite/파일·실제 fixture subprocess** 경로다. 외부 모델의 품질·로그인·요금·지연 증거는 아니다. 실제 Claude/Codex 호출의 별도 통합 기록을 대체하지 않는다.
- 현재 fixture는 완전한 요구사항/coverage를 만들지 않아 검토본 승인 차단과 **초안 저장**이 올바른 판정이다. 승인 가능한 실모델 문서의 사용자 판단이나 모든 오류 장면을 전수 검증했다고 주장하지 않는다.
- Chrome 한 버전, 세 viewport의 표본이다. Firefox/WebKit, 실제 모바일 OS, 스크린리더, 터치 키보드/IME, 장시간 부하·네트워크 장애 전수 검증은 미실행이다. 네트워크 parser·취소·종료 미확인·충돌·응답 유실은 Node 및 Rust의 별도 계약 테스트와 함께 평가한다.
- 작성자와 분리한 검토자가 최종 package/config/browser helper·테스트를 읽기 전용으로 대조했다. 소유 프로세스/context 정리, 비밀 비노출, 실제 저장 파일/next-run 증거, viewport/모달과 7일 retention 범위에서 필수 잔존 지적 없음으로 회신했다. 검토자가 브라우저를 다시 실행했다는 뜻은 아니다.
