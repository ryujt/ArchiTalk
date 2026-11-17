# SmartMD 설계 fixture의 실제 mdview 변환기 호환성

## 사전 범위 — 2026-11-18

- 기준: `main@5954230` 작업 트리. 별도 Developer가 내장 설계 형식 prompt/구조 계약과 `tests/fixtures/architecture.smart.md`를 구현 중이다. 이 기록은 fixture 호환성 테스트 실행 전에 작성했다.
- 목표: 저장된 대표 설계 fixture가 실제 mdview 공개 API에서 navigation/state 변환, jobflow parsing/SVG 생성, layout parsing/HTML 생성, SmartMD frontmatter/callout/table 전처리를 통과하는지 독립 확인한다.
- 소유 파일: `tests/smartmd-compat.test.mjs`(필요시), 이 기록. 원본 project-guides·tools.camp/mdview 저장소는 읽기만 한다. 런타임에 작성자의 절대 경로를 넣지 않으며 테스트 실행 시 `ARCHITALK_MDVIEW_ROOT` 환경변수로 외부 renderer 소스 루트를 전달한다.
- 참고: 기존 [implementation-web-browser](implementation-web-browser.md)와 [chat-scroll-browser-20261118](chat-scroll-browser-20261118.md)는 ArchiTalk 실제 UI 여정의 증거이며 SmartMD 렌더러 호환성을 검증한 기록은 아니었다. 이번 실제 API 검증으로 해당 빈틈만 확인한다. 종합 테스트·project-guides 스킬의 실행 이력/가이드 문법/실행하지 않은 렌더링 성공을 주장하지 않는 규칙을 적용한다.

| 범위 | 실행·판정 방법 |
| --- | --- |
| 실제 기준 | 원본 project-guides의 SmartMD 및 4종 DSL 가이드, mdview `internal/mdview/static/js/{navigation,state,jobflow,layout,smartmd}.js` 공개 API를 읽는다 |
| Navigation / State | 실제 `navigationToMermaid` / `stateToMermaid` 호출, 원본 관계가 변환 결과에 남고 정의된 노드를 연결하는지 확인 |
| Jobflow | 실제 `parseJobFlow`의 객체/관계와 `generateSVG`의 기대 SVG 태그·viewBox·관계 라벨, 비정상 수치 없음 확인 |
| Layout | 실제 `parseLayoutDSL` 트리 및 `generateHTML` 결과의 컨테이너/컴포넌트 확인 |
| SmartMD | mdview vendored `marked.js`와 실제 `preprocessSmartMd` 연결. frontmatter banner, callout HTML, caption/표 사양 JSON marker, GFM 표가 보존되는지 확인 |
| 독립성·한계 | CLI 설치·의존성 추가·원격 API·유료 모델 호출 없이 실행. 이 계층은 변환/파싱/문자열 생성이며 Mermaid SVG 렌더·전체 mdview 브라우저의 실제 시각 검증으로 보고하지 않음 |

## 조사 결과

- navigation/state는 문자열 입력을 `graph LR` Mermaid로 변환하는 export다.
- jobflow default export에 `parseJobFlow` / `generateSVG`가 있고 DOM 의존 없이 SVG 문자열을 생성한다. layout default export는 `parseLayoutDSL`을 제공하며 named `generateHTML`도 제공한다.
- `preprocessSmartMd`는 callout 내부에 `window.marked.parse`를 사용한다. mdview의 vendored marked를 직접 연결하면 가짜 markdown renderer 없이 Node에서 실제 전처리를 실행할 수 있다.
- `applySmartTableSpecs`는 실제 DOM TreeWalker를 요구하는 후처리다. Node 변환 계층에서 table marker 생성까지만 확인하면 DOM 적용/최종 폭·정렬은 별도 미검증으로 구분해야 한다.
- 원본 source 기준: project-guides `51cb5d9`, tools.camp `46dffca`, 둘 다 조사 시 작업 트리 변경 없음. mdview의 vendored marked는 `marked@12.0.2` export이며 `new Marked()`로 실제 parser를 사용한다.
- Developer에게 fixture 저장 완료 통지를 받은 뒤 대표 fixture와 내장 형식 prompt를 읽었다. 목록→상세/실패/재선택/복귀 5관계, Reader/Repository 3관계, 단일 조회 과업 6전이, 목록/상세 layout 2개, frontmatter·info callout·표 사양 1개가 있다.

## 실행 기록

### S01 / R01 — 실제 API 호환성 실행 준비

- 준비: 2026-11-18 08:32 KST. fixture 소유자가 저장을 확정한 뒤 실행한다.
- 명령: `node --check tests/smartmd-compat.test.mjs`, `ARCHITALK_MDVIEW_ROOT="$MDVIEW_SOURCE_ROOT" node --test tests/smartmd-compat.test.mjs`. 이 실행의 환경변수는 조사한 mdview source tree를 가리킨다. 문서/테스트 소스에 작성자의 절대 경로를 저장하지 않는다.
- 판정: 실제 API 5개 하위 검증(navigation, state, jobflow, layout, SmartMD) 통과. 외부 경로 미지정 시 명시적 skip이며 기본 실행이 원본 repo에 의존하지 않아야 한다.
- 시각/결과: **2026-11-18 08:32:00 KST**, 구문 검사 exit 0. 이어 실제 API 실행 **5개 하위 검증 + parent = 6 pass**, fail/skip 0, exit 0, Node 약 **138ms**. SVG·HTML 출력 문자열과 SmartMD 전처리가 통과했다. 브라우저 시각 검증은 실행하지 않았다.

### S02 / R02 — 문서 제목 결합 제거 및 opt-in 경계

- 시작 준비: 2026-11-18 08:32 KST. root가 fixture title/H1을 기존 workflow 기대 제목으로 바꿀 수 있음을 전달했다. 문서 제목 literal assertion을 없애고 fixture frontmatter의 실제 title이 renderer banner에 보존되는지 대조하도록 변경했다. 이 변경은 DSL/SmartMD 구조 검사를 약화하지 않는다.
- 명령: `node --check tests/smartmd-compat.test.mjs`, 외부 루트를 설정한 동일 Node 호환성 명령, `env -u ARCHITALK_MDVIEW_ROOT node --test tests/smartmd-compat.test.mjs`.
- 시각/결과: **2026-11-18 08:32:28 KST** 구문 검사 exit 0. external source 지정 시 **6 pass / 0 fail / 0 skip**, 약 **139ms**, exit 0. 경로 제거 시 **0 pass / 0 fail / 1 skip**, 약 **114ms**, exit 0. 후자는 실제 renderer 검증 통과로 합산하지 않는다.

### R03 — 최종 fixture 제목 변경 후 재확인

- 시작 준비: 2026-11-18 08:32 KST. Developer가 fixture title/H1을 `알림 서비스 아키텍처`로 변경했다고 통지했다. DSL/SmartMD 구조는 같으며 실제 최종 입력을 다시 확인한다.
- 명령: 외부 루트를 설정한 동일 Node 호환성 명령.
- 시각/결과: **2026-11-18 08:32:53 KST** 종료 관찰, **6 pass / 0 fail / 0 skip**, 약 **137ms**, exit 0. fixture 제목 변경에 결합되지 않고 최종 metadata가 실제 banner에 남는 것을 포함해 5개 하위 검증이 통과했다.

## 판정과 남은 경계

- 대표 fixture가 실제 mdview 변환 API와 호환됨을 확인했다. navigation 4노드/5관계, state 5노드/6전이, jobflow Reader/Repository 3관계, layout 2개 page tree와 SmartMD metadata/callout/표 사양이 보존됐다. 실제 marked는 표 사양 대상 2열·2행 GFM 표와 별도 화면 링크 표를 구분했고 전처리 후 5개 DSL 블록을 유지했다.
- **실제 Mermaid SVG 렌더, 전체 mdview viewer/인쇄·내보내기, DOM `applySmartTableSpecs`의 최종 열 폭/정렬, screenshot 시각 QA는 이번 실행에서 미검증이다.** jobflow SVG와 layout HTML은 실제 생성 API의 출력 문자열을 확인했으며 브라우저로 표시했다고 주장하지 않는다.
- 신규 런타임 의존성·CLI 설치·원본 repo 수정·외부 API·유료 모델 호출은 없다. `ARCHITALK_MDVIEW_ROOT` 미지정 skip은 환경 경계를 드러내며 일반 제품 실행이 source repo 경로를 요구하지 않는다.
- 실제 모델이 매번 이 형식을 준수하는지, 잘못된 형식의 거절/저장 계약은 별도 구조/도메인 테스트 담당자의 범위다. 이번 결과는 대표 fixture의 실제 renderer API 호환성 증거다.

### R04 — 실제 Codex 생성 설계의 모든 DSL 변환

- 실행 준비: root의 단일 실제 Codex 생성 probe 완료 통지를 받았다. 입력은 `target/document-probes/365fcc84-3bc8-4134-adfa-fde439fa92de/document.md`이며 이 검증은 새 모델 호출을 만들지 않는다.
- 방법: 임시 Node 스크립트에 document 경로와 mdview source root를 CLI 인자로 전달한다. mdview vendored marked로 실제 document의 모든 code fence를 추출하고 navigation/state/jobflow/layout 블록마다 실제 공개 API를 호출한다. fixture 전용 노드 수·라벨 기대값을 적용하지 않는다. 예외·빈 결과·undefined/NaN/Infinity·연결이 없는 변환 결과를 실패로 판단하며 SmartMD metadata/callout/table 전처리와 DSL 블록 보존도 확인한다.
- 한계: 구조 검증 통과는 root의 실제 생성 test 증거다. 이번 추가 검증도 Mermaid SVG/전체 viewer 시각 검증이 아니다.
- 명령: `node target/document-probes/365fcc84-3bc8-4134-adfa-fde439fa92de/check-mdview-compat.mjs target/document-probes/365fcc84-3bc8-4134-adfa-fde439fa92de/document.md "$MDVIEW_SOURCE_ROOT"`. 마지막 인자만 외부 mdview source root이며 임시 스크립트/제품 코드에 절대 경로를 저장하지 않았다.
- 결과 관찰: **2026-11-18 08:40:26 KST**. 개별 벽시계 시작은 미측정. 명령 exit **0**, 오류 **0건**, shell 도구 측정 약 **7ms**. 원본 **10,693bytes**는 읽기만 했으며 fixture 전용 기대 노드 수 조건은 적용하지 않았다.
- 실제 DSL **7블록 모두 통과**: navigation 2블록(각 **2/4관계**), state 2블록(각 **5/3전이**), jobflow 1블록(**3객체/4관계**, **3,239bytes SVG 문자열**), layout 2블록(목록/상세 루트의 **2/1컨테이너**, **608/397bytes HTML 문자열**). 빈 결과·예외·undefined/NaN/Infinity 없음. navigation/state의 원본 관계 수와 변환 후 관계 수가 각각 일치했다.
- 실제 SmartMD 전처리도 통과: **11,137bytes** 결과, frontmatter banner, **callout 1개**, 파싱 가능한 **table specification 1개**, 실제 GFM 표와 원본 code fence 언어 순서 보존. 보조 Mermaid 블록도 원문 유지 여부에 포함했으나 Mermaid SVG 렌더 검증은 하지 않았다.
- 증거: 같은 probe 폴더의 `mdview-compat.json`(검증 요약), `mdview-conversions.json`(실제 변환 문자열), `check-mdview-compat.mjs`(CLI 인자만 받는 임시 재현 스크립트). Git 제외 artifact이며 요약은 이 이력에 보존한다.
- 판정: root가 생성한 이 실제 문서의 mdview API 변환 호환성 통과. 새 유료 호출·원본 repo/생성물 수정·기존 fixture 테스트 추상화 확장은 하지 않았다. 전체 시각 렌더 경계는 여전히 별도다.
