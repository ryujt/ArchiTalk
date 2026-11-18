# ArchiTalk 문서의 실제 브라우저 렌더링 검증

## 사전 범위 — 2026-11-18

- 기준: `main@5a1a75e`. 사용자 화면의 `.markdown`이 `<pre>` 원문을 표시하는 상태를 먼저 반례로 고정한다. 제품 renderer 연결 전 실제 HTTP·SSE·SQLite·fixture subprocess·새 headless Chrome을 사용한다.
- 목표: 기본 문서 보기에서 Markdown 제목/표, SmartMD metadata/callout/table 사양, navigation/state/jobflow/layout 및 Mermaid가 실제 DOM/SVG로 표시되고 원문 fence가 노출되지 않는다. desktop/mobile 문서 가로 넘침, snapshot 이후 렌더 유지, 잘못된 diagram의 국소 오류와 악성 Markdown 경계를 검증한다.
- 쓰기 소유: `tests/browser/document-rendering.spec.mjs`, 이 기록. 제품/공용 fixture는 root·renderer 담당자가 변경한다. 유료 모델 호출·사용자 브라우저·외부 요청은 없다.
- 이전 이력: [chat-scroll-browser-20261118](chat-scroll-browser-20261118.md) B03/R02는 새 SmartMD의 저장 bytes와 원문 미리보기를 확인했으며 실제 렌더를 주장하지 않았다. [smartmd-renderer-20261118](smartmd-renderer-20261118.md) R04는 실제 생성 문서의 외부 mdview API 변환 문자열까지만 검증했고 Mermaid SVG/브라우저 시각 결과는 미검증이었다. 이번 테스트는 이 경계를 실제 제품에서 확인한다.
- 환경: 기존 browser fixture의 격리된 임시 프로젝트·설정·loopback HTTP/SSE·실제 Rust/SQLite·복사된 Claude executable. desktop 1365×900, mobile 390×844. trace/HAR/video·인증 headers·사용자 profile은 저장하지 않는다. PNG/비밀 없는 DOM 수치만 기존 7일 artifact retention 아래 남긴다.

| 시나리오 | 판정 방법 |
| --- | --- |
| 기본 실제 preview | conversation→design→critic 완료 후 `.markdown` 내부 실제 h1/table/SVG와 source fence 부재 |
| SmartMD·4종 DSL·Mermaid | 실제 fixture·합성 확장 문서의 metadata/callout/table 사양, navigation/state/jobflow SVG, layout DOM, Mermaid SVG |
| 안전한 내용 | 일반 fenced code를 실행하지 않고 표시, HTML/event/JS URL/foreignObject·원격 image/data image 방어, 외부 요청/인증 누출 0건 |
| 실패 복구 | malformed diagram만 오류를 표시하고 앞뒤 Markdown·정상 diagram 유지 |
| 갱신·반응형 | 정상 workspace snapshot 이후 렌더 유지, desktop/mobile 가로 넘침 부재, 실제 PNG 확인 |

## 실행 기록

### S01 / B01 — 제품 수정 전 반례 준비

- 명령 예정: `node --check tests/browser/document-rendering.spec.mjs`, `ARCHITALK_BROWSER_RUN=document-rendering-baseline-20261118 npm run test:browser -- tests/browser/document-rendering.spec.mjs`.
- 첫 baseline은 대표 fixture만 사용한다. renderer 연결 후 추가되는 hostile/Mermaid/malformed 시나리오를 이 실행에서 검사했다고 주장하지 않는다.
- 기대: 현재 `<pre>` preview에는 실제 heading/table/SVG가 없고 raw fence가 표시되므로 desktop/mobile 모두 실패한다. 실패와 screenshot을 남긴 뒤 같은 사용자 결과 assertion을 유지해 재검증한다.

- S01 결과: **2026-11-18 09:01 KST**, `node --check` exit 0.
- B01 결과: **2026-11-18 09:01~09:03 KST**, cargo build **0.37초** 후 desktop/mobile **2건 모두 실패**, exit 1. 재시도 0회. 두 화면 모두 실제 fixture design/critic 완료 뒤 `.markdown` tag=`PRE`, heading/table/SVG **0/0/0**, raw navigation fence 노출=`true`였다. 가로 넘침은 각각 1365/1365px, 390/390px로 없었다. 실제 변환 부재에 따른 **제품 결함**이며 assertion을 약화해 성공 처리하지 않는다.
- B01 증거: `target/browser-artifacts/document-rendering-baseline-20261118/`에 viewport별 `01-document-preview.png/.json`, 실패 PNG/context 보존. desktop/mobile 대표 full-page PNG 두 장을 실제로 열어 frontmatter·callout/table 사양·4종 DSL fence가 원문으로 보이는 것을 확인했다. pageerror·외부 요청·Bearer 위반 0건이었다.
- 다음: 제품 renderer 연결·공용 fixture 주입 helper 확정 후 동일 결과 assertion과 안전성/실패 국소화/갱신 여정을 검증한다.

### S02 / B02 — renderer 연결 후 표적 검증 준비

- 준비: **2026-11-18 09:05 KST**, 신규 spec `node --check` exit 0. root 소유 `server.configureDocumentFixture(markdown)`은 테스트 서버가 복사한 임시 SmartMD 한 파일만 변경한다. 원본 대표 fixture·제품 파일·사용자 저장 문서를 변경하지 않는다.
- 기존 반례의 h1/table/최소 3 SVG/valid DSL raw fence 부재 판정은 유지했다. `.markdown[data-render-state=ready]`, 각 diagram 상태·실제 shape/text·layout DOM·SmartMD metadata/callout/caption/60%·40% 열 폭/왼쪽 정렬, 원문 details 기본 닫힘을 추가한다. 정상 online snapshot 재조회 및 reload→초안 복구 뒤 같은 renderer 결과도 확인한다.
- 추가 hostile 시나리오는 대표 fixture 뒤 합성 appendix를 붙여 실제 fixture subprocess→Rust generation validator→draft→critic→제품 renderer를 통과한다. 일반 HTML fenced code, raw script/event/JavaScript URL/원격·동일 origin API·data image/SVG foreignObject/문서 내부 앱 작업 button·form·중복 ID, Mermaid init/themeCSS/click 주입, 정상 Mermaid, 문법 오류 Mermaid와 뒤쪽 본문을 포함한다.
- 새 세부 시나리오는 B01에서 실행하지 않았다. dedicated DSL의 cycle은 신규 생성 validator가 거절하므로 이번 실제 generation 경로에서는 Mermaid 문법 오류로 국소 실패를 확인한다. 보관된 legacy malformed DSL의 renderer 단위 방어는 별도 제품 검증 범위다.
- 명령 예정: `ARCHITALK_BROWSER_RUN=document-rendering-targeted-20261118 npm run test:browser -- tests/browser/document-rendering.spec.mjs` (2개 시나리오×desktop/mobile=4건). 이 실행에서 실패하면 원인·수정 후 실행을 별도 기록한다.
- S02 보강: 같은 준비 단계에서 hostile case에 실제 request 및 CSP console 거절 관찰도 추가하고 `node --check`를 다시 실행해 exit 0을 확인했다. 차단 정책만으로 안전성 성공을 가리지 않도록 sanitization 이후 DOM에서 금지 node/attribute가 없는지 함께 판정한다.
- S02 추가 보강: 독립 reviewer가 SmartMD fence masking 변경에서 callout 내부 code/diagram의 복원 순서 회귀를 발견했다. root 요청에 따라 합성 `:::note` 안에 일반 JavaScript code fence(`:::info` 문자 포함)와 정상 Mermaid를 추가하고, 실제 nested code 내용·callout 안 SVG/diagram ready를 판정한다. 새 문서 형식 계약은 dedicated DSL을 callout 밖에 요구하므로 nested Mermaid를 사용한다. 구문 검사 재실행 exit 0. renderer 담당자 수정 동결 통지 전에는 B02를 시작하지 않는다.

- B02 결과: **2026-11-18 09:08 KST**, cargo build **4.05초**, Playwright **11.1초**, **2 pass / 2 fail**, exit 1. representative desktop/mobile는 SmartMD/4종 DSL/SVG·layout/표 폭/원문 기본 닫힘/실제 snapshot/reload 복구와 반응형 판정을 모두 통과했다. hostile 두 건은 semantic/security assertion에 도달하기 전 callout title locator가 2개를 찾는 strict violation으로 중단됐다.
- B02 실패 분류: **테스트 결함**. 실제 mdview `renderDirective`는 `:::note`를 `.smart-callout.info`로 매핑한다. 기존 `.info .smart-callout-title` 단일 기대가 base info와 nested note를 동시에 선택했다. 제품 코드 오류로 보고하지 않는다. 대표 callout·nested callout을 title로 식별하게 수정하고 의미/안전성 판정은 유지했다.
- B02 증거: `target/browser-artifacts/document-rendering-targeted-20261118/`의 두 representative PNG·JSON 및 diagram PNG, 두 hostile 실패 PNG/context. hostile의 뒤쪽 검사를 통과했다고 기록하지 않는다.

### S03 / B03 — callout 식별 수정 뒤 표적 재검증

- 명령: `node --check tests/browser/document-rendering.spec.mjs`, `ARCHITALK_BROWSER_RUN=document-rendering-targeted-r2-20261118 npm run test:browser -- tests/browser/document-rendering.spec.mjs`.
- 변경 근거: B02에서 확인한 note→info alias에 맞는 title 기반 locator. 전체 4개 표적 case를 재실행해 helper 공유부도 함께 확인한다. 재시도 정책은 0을 유지한다.

- S03 결과: `node --check` exit 0.
- B03 결과: **2026-11-18 09:09 KST**, cargo build **4.99초**, Playwright **10.2초**, **2 pass / 2 fail**, exit 1. representative desktop/mobile는 다시 통과했다. hostile 두 건은 base·nested callout 일반 code/Mermaid, 정상 Mermaid, malformed Mermaid 국소 오류/뒤쪽 본문, 금지 DOM·event·URL 부재와 probe 실행 false·인증 값 markup 부재까지 통과했다. 마지막 resource-attempt 판정에서 viewport마다 **CSP 거절 console 3건**을 발견했다. 외부 요청이 실제 발신됐다고 단정하지 않지만, 차단 정책 이전의 resource 시도가 없어야 하는 필수 기준을 충족하지 못했다.
- 별도 시각 발견: B02 desktop `01-document-preview.png`를 실제로 열어 일반 렌더 결과를 확인했다. Navigation의 오류 노드가 `ResourceList(LoadError)` 대신 literal `ResourceList&#40;LoadError&#41;`로 표시된다. 실제 diagram label 제품 결함으로 root/renderer 담당자에게 전달했다. 기존 라벨 최소 확인 assertion은 이 문제를 잡지 못했으므로 정확한 label 회귀 검사를 추가할 예정이다.

### D01 — CSP 거절 원인 분리

- 목적: B03의 generic console 계측을 비밀 제거 메시지로 보강해 실제 거절된 resource 종류를 확인한다. 토큰/64자리 hex 값을 제거하고 1000자로 제한한 합성 console만 `safety.json`에 남긴다. DOM/probe boolean도 함께 보존한다.
- 명령: `ARCHITALK_BROWSER_RUN=document-rendering-csp-diagnostic-20261118 npx playwright test tests/browser/document-rendering.spec.mjs --project=desktop --grep='hostile Markdown'`.
- 환경: B03에서 빌드한 실제 제품 binary를 그대로 사용한다. 이 진단은 제품 새 build나 모델 호출을 만들지 않으며 동일 safety 판정 실패를 예상한다. precise 원인을 확인한 뒤 관련 수정·재검증을 별도로 남긴다.

- D01 결과: **2026-11-18 09:10 KST**, desktop hostile **1 fail**, exit 1. 정확한 3건은 모두 **inline event handler 실행을 CSP가 거절**한 메시지였다. network fetch 거절 메시지는 아니었다. 최종 DOM event attribute 부재/probe false만으로 staging 중 안전성을 보장할 수 없다는 제품 결함을 확인했다. renderer 담당자에게 DOMPurify staging 이전의 event/resource 제거 경계를 전달했다. 원인 코드 위치는 당시 가설이며 실제 수정·재검증 전 성공 처리하지 않는다.
- D01 증거: `target/browser-artifacts/document-rendering-csp-diagnostic-20261118/`의 `safety.json`에 최종 DOM/probe boolean과 토큰 제거된 3개 CSP 메시지를 보존했다. pageerror·외부 request·API Bearer 위반 여부는 마지막 실패 assertion 뒤여서 이 진단의 통과 주장에 합산하지 않는다.
- 후속 test 보강: CSP collector의 표시를 resource에 한정하지 않는 `CSP blocked untrusted content`로 정정했다. Navigation은 실제 fixture의 `ResourceList(LoadError)` label이 SVG text에 남는 exact content assertion을 추가했다. root/renderer 담당자가 제품 수정 후 동결을 알리면 같은 4개 표적 case로 검증한다.

### S04 / B04 — staging event와 diagram label 수정 뒤 재검증

- 준비: renderer 담당자가 inert template 원본에서 금지 resource node와 event/id/name/src/style/data attribute를 **DOMPurify staging 이전** 제거하도록 수정하고, SVG text node에서만 제한된 punctuation entity를 복원하는 label 수정 동결을 통지했다. strict Mermaid/htmlLabels false 설정을 유지했다.
- S04 결과: exact navigation error label 및 CSP collector 정정 후 `node --check tests/browser/document-rendering.spec.mjs` exit 0.
- 명령: `ARCHITALK_BROWSER_RUN=document-rendering-targeted-r3-20261118 npm run test:browser -- tests/browser/document-rendering.spec.mjs`.
- 판정: B03/D01 CSP 실행 시도 3건이 0건으로 사라지는지, B02 시각 발견 error node label이 실제 입력대로 남는지 포함해 기존 표적 4건을 모두 검사한다. 실패 assertion을 제거하거나 CSP를 완화하지 않는다.

- B04 결과: **2026-11-18 09:11 KST**, cargo build **3.30초**, Playwright **9.7초**, **2 pass / 2 fail**, exit 1. Navigation exact label 복원은 두 representative/hostile에서 통과했다. 그러나 hostile의 inline event CSP 거절 3건은 두 화면에서 그대로였다. 단순 초기 node/attribute 제거만으로 원인을 해결했다고 주장하지 않는다. 최종 DOM/probe·nested/normal Mermaid·국소 오류는 여전히 통과했다.

### D02 — 정상 문서와 CSP source metadata 대조

- 가설 분리: 앞선 CSP collector는 hostile case에만 연결돼 있었다. trusted renderer의 내부 feature probe인지 untrusted markup staging인지 구분하기 위해 정상 representative에도 같은 console collector를 연결한다. `securitypolicyviolation` event의 directive/blockedURI/sourceFile/line/column/sample도 비밀 제거 후 기록한다.
- 명령: `node --check tests/browser/document-rendering.spec.mjs`, `ARCHITALK_BROWSER_RUN=document-rendering-csp-normal-20261118 npx playwright test tests/browser/document-rendering.spec.mjs --project=desktop --grep='actual document preview'`.
- 제품 binary는 B04 그대로다. 정상 문서에 CSP 거절이 없다면 hostile 단계 문제로 좁히고, 있다면 정확한 source metadata로 trusted vendor 경계를 조사한다. 이 검사는 결과를 보기 전 안전성 원인을 확정하지 않는다.
- D02 결과: **2026-11-18 09:12 KST**, 구문 검사 exit 0, 정상 desktop **1 pass / 0 fail**, Playwright **3.8초**, exit 0. 정상 문서의 최초 render·snapshot·reload 모두 CSP 거절 console 0건이었다. trusted vendor의 정상 시작 probe라는 가설은 이 표본에서 배제하고 hostile markup 경계로 좁혔다.

### D03 — hostile CSP source 위치 확인

- 명령: `ARCHITALK_BROWSER_RUN=document-rendering-csp-source-20261118 npx playwright test tests/browser/document-rendering.spec.mjs --project=desktop --grep='hostile Markdown'`.
- D02와 같은 binary·event collector를 사용하고 source metadata만 확보한다. untrusted source의 원문/토큰 값 대신 제한된 event 필드와 비밀 제거된 console을 보존한다.
- D03 결과: **2026-11-18 09:13 KST**, hostile desktop **1 fail**, exit 1. 3건 모두 `script-src-attr` / `blockedURI=inline`, source=`document-renderer.js`, **line 55 / column 22**였다. 실제 served renderer의 `template.innerHTML = html`에 해당한다. CSP 시도가 main document template parsing 단계에서 이미 발생하므로 DOMPurify 이전 node/attribute 제거도 그 뒤에서는 늦다는 근거다. renderer 담당자와 root에게 전달했다. artifact는 `document-rendering-csp-source-20261118/.../safety.json`이다.
- 추가 시각 확인: B04 mobile `02-navigation.png`, hostile `03-local-error.png`를 실제로 열었다. 오류 노드 parentheses가 읽을 수 있는 문자로 복원됐고 malformed Mermaid의 오류/안전한 source가 해당 figure 안에 한정됐다. mobile 그래프는 화면 폭으로 축소돼 글자가 작다는 관찰을 root에게 선택 개선으로 전달했다. document 가로 넘침이나 겹침은 없었다.

### B05 — 최초 DOM 파싱 경계 교체 뒤 표적 검증

- 준비: root/renderer 담당자가 raw template parsing을 제거하고 DOMPurify string entry를 최초 DOM 파싱으로 사용하도록 수정 동결을 통지했다. SmartMD table comment는 무작위 marker/Map으로 보호해 정화 뒤 신뢰할 marker만 복원하고 검증된 table 사양을 적용한다. 작은 화면 SVG는 figure 내부 가로 이동으로 실제 문자를 읽을 수 있는 크기를 유지하도록 바뀌었다.
- 명령: `ARCHITALK_BROWSER_RUN=document-rendering-targeted-r4-20261118 npm run test:browser -- tests/browser/document-rendering.spec.mjs`.
- 판정: source/CSP 기준·정확한 label·table 폭/정렬·page overflow·nested code/Mermaid·오류 국소화·reload/snapshot 등 기존 4개 case를 유지한다. 이전 실패 3건의 부재를 실제 새 binary에서 확인하기 전 완료로 보고하지 않는다.

- B05 결과: **2026-11-18 09:17 KST**, cargo build **3.09초**, Playwright **10.0초**, **2 pass / 2 fail**, exit 1. 화면/label/table/안전한 최종 DOM·probe 및 diagram 검사는 유지 통과했으나 CSP 3건은 동일했다. source 위치는 DOMPurify의 `DOMParser.parseFromString` (vendored source line 2 / column 21602)로 이동했다. raw template 경계가 제거됐음을 확인했지만 CSP 진단을 실제 handler 실행으로 단정할 수 없으므로 원인 분류를 재검토했다.

### D04 — Chromium 알려진 inert parser CSP 회귀 대조

- 1차 자료 확인: Chromium/WPT의 [2026-10-28 변경](https://chromium.googlesource.com/external/w3c/web-platform-tests/+/refs/tags/merge_pr_64290)은 browsing context가 없는 DOMParser/template document의 inline attribute 설정이 활성 document CSP 위반으로 잘못 보고되는 회귀와 수정(Bug 576126165, Chromium CL 8637073)을 명시한다. 따라서 앞선 CSP 로그만으로 **실제 event 실행 시도가 있었다고 표현한 것은 과한 해석**이었다. 최종 DOM/probe/네트워크 증거와 browser 버전을 함께 판단한다.
- 로컬 환경 확인: `PlistBuddy`로 기본 Chrome **157.0.8103.79**, 이미 설치된 Playwright Chromium cache에서 **158.0.8127.18**을 확인했다. 버전 확인을 위해 사용한 Node private package subpath probe는 `ERR_PACKAGE_PATH_NOT_EXPORTED` exit 1이었으며 테스트/제품 실패가 아니다. 설치·사용자 브라우저 변경은 없다.
- 명령: `ARCHITALK_BROWSER_CHANNEL=chromium ARCHITALK_BROWSER_RUN=document-rendering-csp-chromium158-20261118 npx playwright test tests/browser/document-rendering.spec.mjs --project=desktop --grep='hostile Markdown'`.
- 판정: 같은 B05 제품 binary·동일 hostile 입력·동일 zero-CSP assertion을 새 runner 소유 Chromium에서 검사한다. assertion 예외 필터나 CSP 완화를 넣지 않는다. 새 브라우저에서 사라지면 앞선 실패를 browser regression/oracle 차이로 분류하고 기존 증거를 보존한다.
- D04 첫 실행: browser launch 전 **환경 차단**, exit 1. 설치된 Playwright가 headless shell revision 1255를 요구했지만 로컬 cache는 1254까지였다. 제품/시나리오는 실행되지 않았다. 새 browser 다운로드나 shared cache 변경을 하지 않았다.
- D04 대조 재실행: Git 제외 `target/document-rendering-diagnostic.config.mjs`가 기존 config를 가져오고 `launchOptions.executablePath`만 환경변수의 이미 설치된 1254 headless shell로 지정한다. 테스트·CSP·retries·격리 정책은 동일하다. 명령은 `ARCHITALK_DIAGNOSTIC_BROWSER=<기존 1254 executable> ARCHITALK_BROWSER_RUN=document-rendering-csp-existing158-20261118 npx playwright test --config=target/document-rendering-diagnostic.config.mjs tests/browser/document-rendering.spec.mjs --project=desktop --grep='hostile Markdown'`.
- D04 기존 158 재실행 결과: **2026-11-18 09:20 KST**, hostile desktop **1 fail**, exit 1. 동일한 DOMPurify inert parser 위치의 CSP 3건이 남았다. 설치된 158 branch도 수정 전 동작이므로 version 대조만으로 문제를 없앴다고 주장하지 않는다.

### S05 / B06 — 독립 native probe와 CSP 없는 대조로 oracle 정정

- root 승인 범위: 표준 DOMPurify를 계속 재작성하지 않고 먼저 native no-Window DOMParser probe로 현재 엔진의 false positive를 재현한다. 해당 probe가 실행 없이 재현된 경우에만 `script-src-attr` / `inline` / **고정 DOMPurify `DOMParser.parseFromString` 위치(2:21602)**를 알려진 브라우저 진단으로 구분한다. 다른 CSP·network·event DOM·실행 probe·인증 기준은 유지한다.
- 독립 probe는 외부 resource가 없는 detached button `onclick`을 DOMParser로 파싱하고, `defaultView=null`, 활성 DOM 미삽입, 실행=false를 실제로 확인한다. 이 probe/진단을 `native-parser-probe.json`에 보존하고 실제 제품 여정 전에 관찰 배열을 초기화한다.
- 추가 대조: 같은 격리된 test page의 reload에서 **HTML 응답 CSP header만 테스트 route로 제거**한다. 제품 source/server CSP는 변경하지 않는다. 같은 저장 hostile draft의 모든 DOM/probe/network/인증/diagram 판정을 다시 실행하고, 응답에 CSP가 없는지도 확인한다. 이 조건에서는 CSP exception을 적용하지 않고 console/policy/실행/network 모두 0을 요구한다. 결과는 `04-without-csp-safety.json/.png`에 남긴다.
- 명령: `node --check tests/browser/document-rendering.spec.mjs` exit 0, `ARCHITALK_BROWSER_RUN=document-rendering-targeted-r5-20261118 npm run test:browser -- tests/browser/document-rendering.spec.mjs`.
- 기본 Chrome channel로 돌아와 동일 4개 case를 실행한다. 알려진 CSP 진단을 숨기지 않고 artifact에 남기며, 이전 실패 원인이 브라우저/oracle 차이였는지 이 두 독립 대조 결과로 판정한다.

- B06 결과: **2026-11-18 09:23 KST**, cargo build **0.23초**, Playwright **10.8초**, **4 pass / 0 fail**, exit 0. retries 0.
- 독립 증거: desktop/mobile native parser probe 모두 `noWindow=true`, `attached=false`, `executed=false`인데 CSP inline attribute 진단 **1건**을 재현했다. 제품 없이도 같은 진단이 발생하므로 앞선 CSP-only 실패를 **Chromium 알려진 false positive에 따른 테스트 oracle 결함/환경 차이**로 최종 정정한다. B03/D01 당시 실제 event 실행 시도로 해석한 문구는 이 독립 증거로 대체한다. 이전 실패 로그는 삭제하지 않는다.
- 실제 strict document: 두 viewport 모두 정상 Markdown/SmartMD/4종 DSL/정상·nested Mermaid/국소 오류·readable label·table 폭/정렬·source 기본 닫힘·snapshot/reload·반응형 검사가 통과했다. hostile의 CSP 3건은 독립 native probe가 재현한 경우의 **동일 pinned DOMPurify parser 위치**만 별도 알려진 진단으로 기록됐고, 다른 CSP·외부 request·Bearer·pageerror는 0건이다.
- CSP 없는 대조: desktop/mobile 둘 다 실제 HTML 응답 CSP 부재를 확인하고 같은 hostile draft를 다시 렌더했다. `attributes=[]`, `controls=0`, `executed=false`, `capabilityInMarkup=false`, `unsafeAttempts=[]`, `policyViolations=[]`가 통과했다. 제품의 안전성이 CSP 차단에만 의존하지 않음을 이 입력 표본에서 확인했다. 제품/server CSP는 유지했다.
- 증거: `target/browser-artifacts/document-rendering-targeted-r5-20261118/`의 각 `native-parser-probe.json`, strict `safety.json`, `04-without-csp-safety.json`과 PNG/geometry. 대표 desktop native/no-CSP JSON을 직접 읽어 실제 값도 확인했다.

### B07 — 기존 사용자 흐름 포함 최종 전체 브라우저 회귀

- 명령: `ARCHITALK_BROWSER_RUN=document-rendering-final-20261118 npm run test:browser`.
- 범위: 신규 렌더·안전성 4건 + 기존 긴 대화 scroll/pending/기존 초안 뒤 대화 4건 + settings/workspace 4건 = **12건** 예상. 같은 기본 Chrome·격리된 HTTP/SSE/SQLite·fixture subprocess 환경이다. 기존 suite의 raw source 기대는 root가 semantic heading 판정으로 갱신했다. 새 기능 영향 회귀로 한 번 실행하며, 새로운 변경/실패가 없으면 추가 전체 반복은 하지 않는다.

- B07 결과: **2026-11-18 09:24~09:25 KST**, cargo build **0.07초**, Playwright **1.0분**, **10 pass / 2 fail**, exit 1. 신규 렌더 4건, 기존 긴 대화 첫 시나리오/설정/workspace는 통과했다. 기존 초안 뒤 대화 시나리오 desktop/mobile는 `.markdown`에 raw `# 알림 서비스 아키텍처`를 요구하는 한 legacy assertion에서 중단됐다. 실제 h1과 diagrams는 렌더돼 있었다. **테스트 계약 갱신 누락**이며 scroll 위치 제품 결함으로 분류하지 않는다. 중단 뒤 대화/viewport 판정은 이 실행의 통과에 합산하지 않는다.
- 소유권 확장: root가 run 종료 뒤 `tests/browser/chat-scroll.spec.mjs`의 위 한 줄을 semantic h1 assertion으로 변경하도록 명시 위임했다. 해당 한 줄만 `.markdown h1` / exact 제목으로 수정했고 scroll/pending/viewport 기준은 유지했다. `node --check tests/browser/chat-scroll.spec.mjs` exit 0.

### B08 — legacy marker assertion 갱신 후 최종 전체 회귀

- 명령: `ARCHITALK_BROWSER_RUN=document-rendering-final-r2-20261118 npm run test:browser`.
- 반복 근거: B07에서 새로 발견한 기존 test의 raw marker 기대를 실제 rendered heading 계약으로 갱신했다. 제품 코드는 B06 이후 같으며 신규 안전성/CSP 없는 대조를 포함한 12건 전체가 함께 통과하는지 확인한다. 새 실패가 없으면 전체 검증을 다시 반복하지 않는다.

- B08 결과: **2026-11-18 09:25 KST**, cargo build **0.24초**, Playwright **31.7초**, **12 pass / 0 fail**, exit 0. retries 0. 신규 document 두 시나리오×desktop/mobile와 기존 scroll/pending/초안 뒤 대화/settings/workspace 모든 여정을 실제 격리된 제품 경로에서 통과했다.
- 실제 문서 수치: representative 각 화면 모두 `.markdown`=`DIV`, heading **18개**, GFM table **2개**, navigation/jobflow/state SVG **3개**, layout preview **2개**, valid raw navigation fence 부재. viewport/document width는 desktop **1365/1365px**, mobile **390/390px**로 page 가로 넘침이 없었다. 실제 SmartMD 60%/40% 열 폭·left 정렬과 nested callout code/Mermaid도 통과했다.
- 안전성 최종 판정: strict CSP 아래 알려진 Chromium inert parser 진단만 좁게 구분했다. CSP를 제거한 test page에서는 두 화면 모두 금지 DOM/attribute 0, 앱 action/ID 충돌 0, 실행 probe false, capability markup 노출 false, 외부/자동 resource 요청 0, policy 진단 0이었다. 모든 기존 fixture Bearer/pageerror/외부 요청 경계도 통과했다. 이 입력 표본에 대해 통과한 것이며 임의의 모든 공격을 증명했다고 주장하지 않는다.
- 산출물: `target/browser-artifacts/document-rendering-final-r2-20261118/`에 PNG **50장**, 문서/대화 geometry JSON, 독립 native probe·strict CSP 분류·CSP 없는 대조 JSON을 보존했다. 기존 7일 retention, trace/HAR/video 비기록, 토큰 제거 정책을 유지했다.

### R01 — 최종 시각 확인과 범위 종료

- **2026-11-18 09:26 KST**, B08 desktop `01-document-preview.png`와 mobile hostile `03-local-error.png`를 실제로 열었다. 제목·metadata·callout·표·navigation/jobflow/state·두 layout의 실제 표시와 원문 details 닫힘, malformed Mermaid의 figure 내부 오류/안전한 source 표시를 확인했다. B06 mobile navigation/desktop Mermaid crop도 실제 열어 자연 크기의 readable label과 figure 내부로 한정된 표시를 확인했다. 큰 graph의 page 전체 가로 넘침이 없다는 geometry 판정도 대조했다. 실제 터치·수평 휠 gesture는 이번 시각 확인에서 직접 조작하지 않았다.
- 미해결 필수 제품 결함은 이번 검증 범위에서 발견하지 못했다. B01 원문 반례, B02 locator 결함, B03~B05/D01~D04 CSP oracle 조사와 환경 차단, B07 legacy marker 결함의 원본 결과를 모두 보존했다. 마지막 실행 B08이 통과한 상태로 source를 동결하고 root에게 전달한다.
- 한계: 실제 유료 모델/사용자 계정·기존 브라우저·실제 모바일 OS/IME·모든 브라우저 엔진/임의 악성 입력을 검증하지 않았다. 모델 subprocess만 명시적 합성 fixture이며 HTTP/SSE·SQLite·파일 저장·UI는 실제 경로다. DOMPurify 버전을 바꾸면 buggy Chromium의 좁은 parser 위치 판정도 새 source/근거와 함께 검토해야 한다. Chromium 수정 브랜치에서는 native probe가 진단을 만들지 않으므로 예외 없이 CSP 0을 요구한다.

- 소유 파일 최종 diff 확인: `git diff --check -- tests/browser/chat-scroll.spec.mjs` exit 0. 기존 chat spec 변경은 위임된 semantic heading 한 줄뿐이다. 진단용 임시 config는 검증 뒤 제거했고 모든 run 증거/artifact는 보존했다. 제품/vendor/package 수정이나 commit은 Tester가 수행하지 않았다.
