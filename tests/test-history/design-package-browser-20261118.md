# Method-R 설계 10문서 패키지 브라우저 검증

## 사전 범위 — 2026-11-18

- 기준: `d35a386` + 신규 패키지 backend/UI. 이전 [document-rendering-browser-20261118](document-rendering-browser-20261118.md) B08의 12건 통과와 CSP oracle 조사 결과를 계승한다. 새 공개 계약은 main Markdown + 9 typed companion이며 서버 catalog의 고정 파일명으로 하나의 개정을 저장한다.
- 목표: Method-R 4깊이·driver→QA→대안→ADR→검증의 연결을 실제 문서 선택/링크/anchor로 읽고 10개 파일 전체를 hash/manifest와 함께 저장·복구한다. 렌더링·보안·긴 대화/pending/기존 초안 뒤 대화·설정 회귀를 유지한다.
- 소유: `tests/browser/*.spec.mjs`, 이 기록. 공용 server fixture/CLI glue/README 및 제품·storage·renderer는 다른 담당자 소유다. root가 UI 동결 후 실행을 허용했다. 모델은 격리된 fixture subprocess이며 유료 호출/사용자 브라우저/기존 CDP를 사용하지 않는다.
- 이전 저장 검사의 ‘main에 모든 4DSL, 파일 1개’는 폐기된 출력 계약이다. 새 main의 Method-R jobflow 2개/50:50 SmartMD 표·scope callout을 검사하고 실제 문서 picker로 전용 4문서의 DSL SVG/layout을 확인한다. 저장은 catalog 10파일과 실제 bytes/각 hash를 검사한다. 기존 범위를 숫자만 줄여 성공시키지 않는다.
- security appendix의 Markdown JavaScript URL은 새 package link validator가 생성 단계에서 거절하므로 raw HTML JS URL은 실제 generation 경로에 유지하고, Markdown JS URL renderer 방어는 같은 실제 browser의 공개 renderDocument API에 붙인 임시 DOM으로 따로 확인한다. final DOM/probe/network/auth 기준 및 독립 native parser/CSP 없는 격리 대조를 유지한다.
- legacy는 root 결정대로 backend 실제 후방 호환 계약 + UI unit fallback의 조합을 사용한다. 테스트가 사용자/공용 SQLite를 직접 변조하거나 legacy browser seed를 추가하지 않는다.

| 범위 | 실제 판정 |
| --- | --- |
| 문서/내용 | catalog 고정 10 kind/파일명, picker 10개, main 4 Method-R heading, 사용자 근거 인용, 5개 trace 문서의 정의·참조, 전용 DSL SVG/layout |
| 링크 | main 9개 링크, nav↔layout/부모 계약, 같은 문서·중복 heading, URI-encoded NFD 한국어 파일/anchor→NFC, 알 수 없는 파일 비활성, `.md` GET 0 |
| snapshot/복구 | 같은 선택 문서의 실제 DOM node 보존, reload 후 새 Run 없이 모든 종류 재선택 |
| 실제 저장 | companion 선택 중에도 전체 10파일 preview/apply, 상대 고정 파일명/bytes/hash/manifest/source package hash 일치 |
| 회귀 | 기존 12건의 scroll/pending/settings/workspace와 renderer security/CSP 없는 대조, desktop/mobile no page overflow |

## 실행 기록

### S01 — 새 계약의 구문 검사

- `node --check tests/browser/document-rendering.spec.mjs`, `node --check tests/browser/workspace.spec.mjs`, `node --check tests/browser/design-package.spec.mjs`가 작성 단계에서 exit 0이었다. 실행 직전 최종 구문을 다시 확인한다. 아직 browser 통과를 주장하지 않는다.
- 표적 예정: `ARCHITALK_BROWSER_RUN=design-package-targeted-20261118 npm run test:browser -- tests/browser/design-package.spec.mjs tests/browser/document-rendering.spec.mjs tests/browser/workspace.spec.mjs` (3개 spec × desktop/mobile = 8건). 통과 후 기존 scroll/settings 포함 전체 14건을 한 번 실행한다.

### B01 — 동결 후 표적 8건

- 명령: `ARCHITALK_BROWSER_RUN=design-package-targeted-20261118 npm run test:browser -- tests/browser/design-package.spec.mjs tests/browser/document-rendering.spec.mjs tests/browser/workspace.spec.mjs`.
- 결과: cargo build **5.83초**, Playwright **40.3초**, **8 pass / 0 fail**, exit 0, retries 0. 첫 browser 실행에서 통과했으며 중간 assertion 수정/재시도는 없었다.
- 실제 범위: 두 viewport 모두 10개 catalog/picker, main 4 Method-R heading과 사용자 인용, 9개 문서의 실제 main-link 진입, trace 정의·상호 참조, nav↔layout/부모·동일·중복·NFD 한국어 anchor focus, 같은 문서 snapshot DOM identity, companion 선택 중 전체10파일 저장/hash/manifest 및 reload 복구가 통과했다. `.md` GET·외부 request·Bearer·pageerror 0건이다.
- renderer 회귀도 새 principal/table 계약과 실제 companion 선택을 통해 4DSL SVG/layout을 확인했다. 일반/nested code·Mermaid, 국소 오류, JS URL API negative, native CSP false-positive 분류와 test-only CSP 제거 뒤 최종 DOM/probe/network 0 기준이 유지됐다.
- 증거: `target/browser-artifacts/design-package-targeted-20261118/`의 principal/linked-layout/saved-package PNG, 10 file hash JSON, renderer/CSP strict·CSP 없는 대조 artifacts 및 기존 workspace screenshots. semantic 내용의 적합성은 fixture 전제이며 실제 모델의 설계 품질 판정으로 바꾸지 않는다.

### B02 — 기존 12건 포함 최종 전체 14건

- 명령: `ARCHITALK_BROWSER_RUN=design-package-final-20261118 npm run test:browser`.
- 근거: 새 package 저장·선택 경계가 기존 대화/pending/설정의 직접 소비자이므로 표적 통과 후 전체 14건을 한 번 검증한다. 새 변경·실패가 없으면 반복하지 않는다.

- B02 결과: cargo build **0.17초**, Playwright **52.4초**, **13 pass / 1 fail**, exit 1, retries 0. mobile hostile는 새 raw HTML block 거절 오류로 generation 단계에서 중단됐다. 실패 context에서 `raw HTML blocks are not supported in new design packages`를 확인했다. desktop 같은 hostile는 통과했다.
- 원인/경계: root의 독립 검토가 raw `<pre>`로 문서 전체를 감싸 필수 heading 검사를 통과하는 거짓 양성을 발견해 실행 도중 new profile의 CommonMark HTML block-start 거절을 추가했다. 각 case가 같은 mutable `target/debug/architalk`를 새로 시작하므로 desktop/mobile가 서로 다른 binary 계약을 관찰한 실행이었다. 테스트/제품 렌더 실패나 안정적인 최종 검증으로 해석하지 않는다. 새 validator는 필요한 보강이며 이전 결과를 삭제하지 않는다.

### S02 — 새 raw HTML block 경계에 맞춘 유효 공격 입력

- root·validator 담당자와 합의해 모든 actual generation HTML 공격 및 NFD/unknown-file `<a>`를 `검증 입력: <...>`처럼 명시적 문단 접두어 뒤 **inline HTML**로 이동했다. 실제 untrusted script/event/image/JS URL/form/SVG payload는 보존했다. fenced code 원문은 변환하지 않으며 독립 renderer API negative와 CSP 없는 대조도 유지한다.
- `node --check tests/browser/document-rendering.spec.mjs`, `node --check tests/browser/design-package.spec.mjs` exit 0. 새 source 동결 뒤 최종 전체 14건을 다시 실행한다. 반복 근거는 validation 경계가 바뀐 점과 B02 mixed binary 실패이며 assertion/timeout을 약화하지 않는다.
- 실행 안정성: root에게 run별 immutable binary 경로 또는 cargo build 동결을 요청했다. 브라우저 snapshot 하나가 suite 전체의 제품 source 버전을 대표한다고 추정하지 않는다.

### B03 — HTML block 경계 반영 후 고정 binary 최종 14건

- root가 `ARCHITALK_BROWSER_BINARY` override를 공용 fixture에 추가했다. `cargo build --bin architalk` **3.36초**, exit 0 후 이번 run 소유 `target/browser-binaries/design-package-final-r2-20261118/architalk`로 복사했다. 크기 **32,621,592 bytes**, SHA-256 **2cd33268da76514ba7c07576c57295b9b5a4e9d2a7b0d3bad4c30f3d5cd763d8**이며 같은 폴더 `binary.json`에 기록했다. Playwright output cleanup/동시 root cargo build가 복사본을 바꾸지 않는다.
- 명령: `ARCHITALK_BROWSER_BINARY=<이번 run 소유 복사본> ARCHITALK_BROWSER_RUN=design-package-final-r2-20261118 npx playwright test`.
- 조건: 새 HTML block guard/최종 prompt를 포함한 고정 제품 binary, inline HTML로 이식한 안전성 입력, 동일 desktop/mobile 14건. 제품 source/CSP를 완화하거나 실패 재시도를 켜지 않았다.

- B03 결과: **2026-11-18 09:44 KST**, Playwright **36.2초**, **14 pass / 0 fail**, exit 0, retries 0. 고정 binary에서 HTML block 새 경계/실제 inline 공격·CSP 없는 대조·10문서 연결/저장/복구와 기존 scroll/pending/settings를 함께 통과했다. B02 중간 source 교체 실패도 원본 그대로 보존했다.
- 최종 artifact: `target/browser-artifacts/design-package-final-r2-20261118/` PNG **56장**, 실제 저장 10종의 byte hash JSON, native parser/strict CSP 분류/CSP 없는 대조 JSON. desktop/mobile 각각 저장 이름 **10개·중복 0개**이며, CSP 없는 조건의 `attributes=[]`, `controls=0`, `executed=false`, `capabilityInMarkup=false`, `unsafeAttempts=[]`, `policyViolations=[]` 값을 직접 읽어 확인했다.
- 시각 QA: B01 desktop `01-principal.png`, mobile `02-linked-layout.png`/`03-saved-package.png`를 실제로 열어 Method-R macro/module jobflow·9개 문서 map·분리된 종류/버전 선택·두 layout·전체10저장 경로를 확인했다. B03의 mobile `02-linked-layout.png`, desktop `03-saved-package.png`도 실제 열어 최종 표시와 줄바꿈/전체 저장 문구를 확인했다.
- 소유 tracked diff `git diff --check` exit 0. 제품·vendor·package·공용 helper/CLI 수정이나 commit은 Tester가 수행하지 않았다. 이번 범위의 필수 잔존 UI 결함은 발견하지 못했다. 실제 모델 설계 품질·계정/실제 모바일 OS/IME·다른 엔진·legacy 실제 browser seed는 위에 명시한 별도 검증/한계로 남긴다.

### S03 — 실제 생성 응답 opt-in renderer probe 준비

- root가 진행 중인 실제 Codex 단일 응답을 새 모델 호출 없이 추가 검증하도록 요청했다. `ARCHITALK_RENDER_PACKAGE_JSON`이 있을 때만 별도 `generated-package.spec.mjs`를 실행하며 없으면 명시 skip한다. 기존 14건을 다시 실행할 필요는 없다.
- 범위: fixture HTTP 서버의 static renderer/vendor/CSS와 테스트 전용 HTML harness에서 공개 `renderDocument`를 직접 호출해 실제 응답 10문서를 각각 렌더한다. 제품 app.js·workspace/생성 API·저장 상태는 사용하지 않는다. source diagram fence 수와 실제 figure 수/ready, 국소 오류 0, no page overflow/pageerror/외부 요청 및 실제 PNG를 기록한다.
- 실행은 root가 검증할 JSON 경로와 응답 형식을 확정한 뒤 시작한다. 준비 단계에서 실제 생성물의 변환/브라우저 성공을 주장하지 않는다.
- S03 구문 결과: `node --check tests/browser/generated-package.spec.mjs` exit 0. root가 원본 `target/document-probes/3b881d77-4d6b-4109-8db1-29dacaa6b98f/response.json`을 지정했다. 10개 문서 본문 UTF-8 합 **46,446 bytes**, 원래 paid 호출 1회는 root의 실행이며 Tester가 다시 호출하지 않는다. 원본을 수정하지 않는다. 아직 probe 실행 전이다.

### S04 — 작업 시간 제한 후속 UI 검사 준비

- root/renderer 담당자가 서버 default 300초와 UI `#timeout-seconds` (required 정수 10–600, step 1, 다음 작업 적용)를 추가했다. 기존 settings 2개 viewport case에 현재 서버 값 노출, 9/601/10.5/빈 값 native 거절·PATCH 0, 숫자480 PATCH/저장/reload, 기존 Run 불변을 추가한다.
- 실제 fixture의 3초 지연 conversation이 이미 240초 spec으로 실행 중일 때 설정을120으로 바꾸고 in-flight effective_settings 불변을 확인한다. 완료 후 다음 conversation이120을 사용하며 최초3개 Run이 그대로인지 확인한다. 유료 모델 대신 기존 격리 fixture만 사용한다.
- `node --check tests/browser/settings.spec.mjs` exit 0. root의 마지막 source 동결 신호 뒤 settings 2건과 opt-in 실제 생성 renderer 2건만 표적으로 실행한다. 기존14 전체를 반복하지 않는다.

### B04 — 실제 보존 응답 렌더 + 시간 제한 표적 첫 실행

- root가 모듈 도표의 유효한 companion anchor 연결을 허용하는 최종 source 동결을 통지했다. `cargo build --bin architalk` **3.12초**, exit 0 후 `target/browser-binaries/design-package-followup-20261118/architalk`로 고정했다. SHA-256 **f23c6c306ba49b09cf4f5e473935c3b04f9310ebb0cd2661c24976c17ebefdcd**, 크기 **32,716,200 bytes**.
- 명령: `ARCHITALK_BROWSER_BINARY=<고정 복사본> ARCHITALK_RENDER_PACKAGE_JSON=target/document-probes/3b881d77-4d6b-4109-8db1-29dacaa6b98f/response.json ARCHITALK_BROWSER_RUN=design-package-followup-20261118 npx playwright test tests/browser/settings.spec.mjs tests/browser/generated-package.spec.mjs`.
- 결과: Playwright **8.0초**, **2 pass / 2 fail**, exit 1. 실제 생성 renderer desktop/mobile 두 건은 통과했다. root가 앞서 받은 단일 실제 응답을 read-only로 검증한 것이며 Tester 추가 paid 호출은 **0회**다. 원본 응답은 수정하지 않았다.
- 실제 응답 검증: 각 viewport에서 원본 10문서·source diagram 총12개를 public renderer로 표시했고 ready·오류 0·figure 종류/수 일치·실제 shape/geometry·page overflow 없음·pageerror/외부/API 요청 0을 통과했다. static harness는 app.js나 app state/generation API를 호출하지 않았다. actual PNG 20장과 두 `generated-render-summary.json`을 보존했다.
- settings 실패 분류: **테스트 observer 결함**. PATCH envelope는 `payload.settings.ai.timeout_seconds`인데 direct `settings.ai...`를 읽어 observer TypeError가 났다. 서버 실제 저장값 480은 이미 확인됐으나 후속 in-flight/next-run 판정 전 중단됐다. observer 경로만 수정했으며 input/PATCH/기존·진행중 Run/다음 Run assertion을 유지했다. `node --check tests/browser/settings.spec.mjs` exit 0.

### B05 — observer 수정 뒤 시간 제한 2건 재검증

- 명령: `ARCHITALK_BROWSER_BINARY=<같은 f23c6c30... 고정 복사본> ARCHITALK_BROWSER_RUN=timeout-settings-final-20261118 npx playwright test tests/browser/settings.spec.mjs`.
- 원인 관련 settings desktop/mobile만 재실행한다. 통과한 실제 생성 renderer 2건/기존14건을 반복하지 않는다. 모델 호출·실제 응답 재작성은 없다.

- B05 결과: **2026-11-18 09:57 KST**, Playwright **22.8초**, settings desktop/mobile **2 pass / 0 fail**, exit 0, retries 0. PATCH는 실제 envelope의 숫자 **[480, 240, 120]**을 관찰했다. native invalid 네 입력은 요청을 만들지 않았고 480 저장/reload, 기존3 Run 불변, 240으로 시작한 in-flight 요청의 설정 고정, 이후 새 요청의120 적용을 확인했다. observer 오류는 B04에 실패로 보존했고 제품 코드는 바꾸지 않았다.
- 실제 생성 요약 직접 확인: 두 `generated-render-summary.json` 모두 문서 **10개**, 본문 **46,446 bytes**, source fence **12개**와 실제 diagram **12개**, state 전부 ready, 오류 **0**, page overflow false였다. 생성물 원본 JSON 현재 SHA-256은 **b7f960bc1ecb8bc6ab3802cfc1f5f70bf009d22cb19d2785bce28dbe13d8a428**이며 테스트가 원본을 쓰는 코드는 없다. 추가 paid 호출 **0회**/원본 수정 **0회**를 유지했다.
- 후속 시각 확인: B04 실제 응답 desktop `architecture.png`, mobile `jobflow.png`를 실제로 열어 합성 fixture와 다른 실제 응답의 SmartMD 표/callout/한글 Method-R jobflow와 본문을 확인했다. B05 mobile `04-timeout-applied.png`에서120 값·10~600초 정수·다음 작업 안내와 모바일 dialog 줄바꿈을 확인했다.
- 최종 판정: 패키지 전체 **14건(B03)** 통과 뒤, 후속 변경은 실제 보존 응답 renderer **2건(B04)**과 시간 제한 settings **2건(B05)**으로 각각 통과했다. B04 최초 4건 실행은 2 pass/2 fail이었고 두 번의 별도 실행을 하나의 4 pass 실행으로 기록하지 않는다. 새 optional spec은 ENV가 없으면 선언 단계 skip이며 app/model 요청을 하지 않는다.
- 최종 source 및 이력을 동결해 root에 전달한다. B03의 56 PNG, B04 실제 응답20 PNG, B05 설정 PNG와 실패 증거를 기존 retention/토큰 비기록 정책 아래 보존했다. 브라우저 검증자는 commit/push/설치/새 유료 모델 호출을 수행하지 않았다.
