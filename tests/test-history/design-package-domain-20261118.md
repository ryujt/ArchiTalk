# 설계 패키지 도메인 계약 검증

- 기준: `main@d35a386` 문서 renderer 커밋과 신규 package 모듈·prompt·fixture 미커밋 변경. 이 담당자는 `src/design_package.rs`, `src/document_format.rs`, `src/domain.rs`, 신규 package prompt·fixture·전용 테스트를 소유한다. 모델·파일·앱·browser는 다른 담당자 소유다.
- 이전 이력: [기존 SmartMD 생성](smartmd-documents-20261118.md), [대화·문서 통합](conversation-docs-integration-20261118.md) INT-01/INT-REAL-01, [문서 renderer](document-rendering-browser-20261118.md)를 읽었다. 기존 한 문서 validator·보관 초안 export는 유지하고 새 생성 경로만 10개 문서 패키지로 전환한다. 과거 실제 모델 성공은 새 패키지 계약의 성공으로 대신하지 않는다.
- 역할: 상속 모델의 도메인 Developer가 구현·경계 테스트를 수행하고 root와 별도 저장 담당자가 계약을 검토한다. 신규 모델 호출·설치·커밋은 하지 않는다. 실제 외부 모델의 새 JSON 생성 가능성은 이번 합성 계약 테스트 범위 밖이다.
- 계약: 엄격한 `markdown`+9종 `documents` DTO, 서버 파일명, 문서별 150000/전체 500000 UTF-8 byte 한도, 공통 SmartMD·타입별 H2/DSL, Method-R4깊이·매크로/모듈 jobflow, 소유 문서의 DRV/QA/ALT/ADR/VAL 정의·참조와 로컬 문서 링크·anchor, 전체 critic 문맥 및 예산 초과 명시 실패다. legacy validator의 네 DSL 의무는 새 지원 문서에 전파하지 않는다.

## PKG-D01 — 구현 전 계약·실행 준비

- 상태: 2026-11-18 KST 준비 완료. renderer 최종 browser 12건 통과 및 binary 보존 신호를 받은 뒤 wired 소스 변경을 시작했다. 그 전에는 신규 미연결 모듈·prompt·fixture·테스트만 준비했으며 cargo를 실행하지 않았다.
- 대상: 합성 자료 조회 패키지를 정상 입력으로 사용하고 누락·중복·알 수 없는 종류·임의 파일명, 잘못된 trace·링크·anchor, companion 끝부분 critic 누락, 과도한 크기를 반례로 사용한다.
- 판정: 신규 package tests와 기존 legacy 형식·도메인 회귀가 통과해야 한다. 실제 model/운영 성능·사용자 승인을 주장하지 않는다. 실행 명령과 종료 결과는 후속 ID에 별도 기록한다.

## PKG-D02 — 신규 패키지·legacy 도메인 계약

- 시작: 2026-11-18 KST. 기준 `d35a386`+담당 신규 validator/profile/prompt/fixture, 저장 담당의 Draft.documents·app·schema2 변경. 이전 D01 계약 준비와 기존 INT-01을 참고한다.
- 명령: `cargo test --locked --test design_package --test document_format --test domain_contract`. 유료 모델 호출은 없는 합성 JSON·로컬 도메인 검증이며 기존 형식 테스트의 macOS fixture subprocess도 실행한다.
- 기대: 신규 package/크기/trace/link/fullcritic/hash/legacyreview와 기존 한 문서 validator·인터뷰 계약 통과. 상태: 실행 중.

- D02 종료: 컴파일 실패(exit101), `DocumentKind`의 Hash derive 누락 1건. 테스트 실행 0건이며 통과가 아니다. D03에서 derive 보완 후 같은 계약을 실행한다.

## PKG-D03 — Hash 보완 후 도메인 계약

- 시작: 2026-11-18 KST. D02 컴파일 원인을 `DocumentKind: Hash` derive로 보완했다. UI 담당과 anchor의 NFC·표시문 기준을 조율하고 inline link/일반 HTML·entity 표시문 전처리 및 한글 NFD 테스트를 추가했다.
- 명령: `cargo test --locked --test design_package --test document_format --test domain_contract`. 기대·범위는 D02와 같고 신규 anchor 경계가 추가된다. 상태: 실행 중.

- D03 종료: **37건 통과 / 0 실패 / 0 ignored**, exit0. 신규 package13건, legacy 형식11건(실제 fixture subprocess 실패 경로 포함), 인터뷰 도메인13건이 각각 0.03s/0.60s/0.00s에 통과했다. 컴파일5.88s. `tests/document_format.rs`에서 패키지 RULES로 바꾼 뒤 남은 unused import 경고1건은 해당 파일 소유 저장 담당에게 제거 요청했다. 실제 모델 생성·운영 측정은 미실행이다.

## PKG-D04 — 담당 라이브러리·테스트 lint

- 시작: 2026-11-18 KST. D03 동작 통과 후 `cargo clippy --locked --lib --test design_package -- -D warnings`로 새 validator/profile/domain과 신규 테스트의 lint를 확인한다. 다른 담당자가 쓰는 기존 테스트 파일은 이 명령에 포함하지 않는다. 상태: 실행 중.

- D04 종료: lint 실패(exit101), reference link parser의 불필요한 `trim().split_whitespace()` 1건. 기능 테스트 실패가 아니며 D05에서 `trim()` 제거 후 재검증한다.

## PKG-D05 — lint 수정·SmartMD 지원 경계 재검증

- 시작: 2026-11-18 09:33 KST. D04 lint 원인을 제거했다. 지원 문서에 비표준 callout이 들어오면 거절하도록 새 profile을 보강했고 scalar frontmatter/unsupported callout 반례를 기존 테스트에 추가했다. legacy profile 의미는 유지한다.
- 명령: `cargo clippy --locked --lib --test design_package -- -D warnings`, `cargo test --locked --test design_package --test document_format --test domain_contract`. 새 변경의 회귀와 정적 검사를 순차로 확인한다. 상태: 실행 중.

- D05 종료: **2026-11-18 09:33:04 KST 결과 확인**, clippy exit0(1.44s), 기능 **37건 통과 / 0 실패 / 0 ignored**, exit0(컴파일1.79s; package13/legacy11/domain13). 기존 테스트 unused import 경고1건 제거는 그 파일 담당자의 변경 대기이며 담당 범위 lint는 경고0건이다. 추가 구조 반례가 통과했고 기존 legacy 의미가 유지됐다.

## PKG-D06 — heading 표시문 anchor 경계

- 시작: 2026-11-18 09:33 KST. UI 담당과 일치 검토 중 Markdown `_강조_`와 코드의 literal underscore를 구분해야 함을 확인했다. 새 helper에서 이스케이프·inline code·underscore 강조를 구분하고 기존 NFD/표시문 테스트에 반례를 추가했다. 이 경계 수정은 driver/legacy 프로필과 무관하며 신규 package 검사·lint만 재실행한다.
- 명령: `cargo test --locked --test design_package`, `cargo clippy --locked --lib --test design_package -- -D warnings`. 상태: 실행 중.

- D06 종료: **2026-11-18 09:34:07 KST 결과 확인**, 신규 package **13건 통과 / 0 실패 / 0 ignored**(0.04s; 빌드대기 포함 컴파일2.59s), focused clippy exit0(1.25s). 기존 D05 37건 통과 이후 바뀐 anchor helper 경계와 신규 반례가 통과했다. 필수 도메인 검사 완료, root의 최종 조립·독립 검토 및 browser 연결 검증은 별도다.

## PKG-D07 — 독립 검토의 raw HTML 위장 반례

- 시작: 2026-11-18 09:39 KST. 원래 Domain 작성자와 다른 검토자가 `outline_lines`와 profile scanner의 HTMLblock 경계 누락을 발견했다. 유효 패키지의 Drivers 문서에서 frontmatter 뒤 본문을 `<pre>`로 감싸면 실제 Markdown heading이 없는 HTMLblock인데 필수 H2/DRV 정의로 계수한다.
- root가 이 반례의 수정 파일(`src/document_format.rs`, package prompt·테스트·이력)만 검토자에게 위임했다. 이 수정 이후 해당 부분의 검토는 독립 검토로 부르지 않는다. Storage 작성 부분의 자기검토와도 구분한다.
- 사전 명령: `cargo test --test design_package required_sections_cannot_be_hidden_inside_raw_html_blocks -- --nocapture`. 먼저 현재 제품의 거짓양성으로 테스트가 실패하는지 확인한다. 실제 모델 호출·사용자 파일·원격 리소스는 없다. 상태: 실행 직전.

- D07 결과: **2026-11-18 09:39 KST**, **0 pass / 1 fail / 14 filtered**, exit101. 실제 신규 validator가 `<pre>`로 감싼 Drivers의 필수 headings를 인정해 반례 assertion이 실패했다. 테스트 실행은 합성 입력을 제품 공개 validator에 직접 전달했으며 거짓양성을 확인했다.

## PKG-D08 — 신규 profile의 HTMLblock 제한과 기존 계약 회귀

- 수정: 새 profile과 package outline scanner에서 raw HTML block 시작을 보수적으로 거절한다. CommonMark 0.31.2 HTML blocks의 시작 조건을 참고했다. legacy profile은 변경하지 않는다. package prompt에 실제 Markdown/SmartMD 사용과 HTML 예제의 fence 사용을 명시했다. inline span/b, 비교문, inline·fenced·indented code가 허용되는 반례도 추가했다.
- 명령: 소유 세 Rust 파일 형식 검사와 `cargo test --test design_package --test document_format --test domain_contract`, `cargo clippy --lib --test design_package -- -D warnings`. 상태: 실행 직전.

- D08 결과: **2026-11-18 09:42:09 KST 결과 확인**, 실제 형식 명령은 수정한 Rust 두 파일(`src/document_format.rs`, `tests/design_package.rs`)의 `rustfmt --edition 2024`였으며 exit0. 신규 package **15건**·기존 한 문서 **11건**·인터뷰 도메인 **13건**, 합계 **39 pass / 0 fail / 0 ignored**, cargo exit0. focused clippy exit0·경고0건. HTMLblock 7종 거절과 inline markup/비교/코드 허용이 함께 통과했다. 공통 시작 조건은 [CommonMark 0.31.2 HTML blocks](https://spec.commonmark.org/0.31.2/#html-blocks)를 참고했고, 완전한 Markdown parser 적합성을 주장하지 않는다.
- 원래 browser hostile HTML은 신규 생성 계약의 금지 block과 혼동하지 않도록 tester가 보통 문장 접두어의 inline 공격으로 바꾸며, 직접 renderer 보안 검증도 별도로 유지하기로 통지했다. 이 browser 수정·결과는 tester의 이력에 남긴다.
- 수정 파일은 root에 동결 통지했다. root가 별도 공유 DSL syntax sheet를 package prompt에 추가하고 최종 조립·실제 모델 probe를 수행한다. 이번 D07/D08은 실제 모델을 호출하지 않았다.
- 추가 정적 확인: 같은 D08 상태에서 `git diff --check` exit0. UI 원본의 서버 catalog 기반 로컬 링크 callback·기본 navigation 차단·session/draft/연결 세대 검토에서 추가 critical/major blocker는 발견하지 못했다. Storage와 이번 HTMLblock 수정은 이 검토자의 작성 영역이므로 독립 검토로 계산하지 않는다. 전체 품질·browser 조립 판정은 root가 담당한다.

## PKG-D09 — 실제 응답의 모듈 도표 위임과 중복 금지 계약

- 배경: root의 단일 실제 Codex 호출은 10문서 46,446 bytes를 반환했다. 주 문서의 macro scope 도표는 있고 module 절은 `잡플로우.md#자료-조회와-검증`의 실제 orchestrator 도표로 연결했다. 주 문서에도 같은 module 도표를 강제한 검사가 중복 금지·상세 문서 연결 계약과 충돌했다. 원본 응답은 `target/document-probes/3b881d77-4d6b-4109-8db1-29dacaa6b98f/response.json`에 보존되어 있다.
- 사전 명령: `cargo test --test design_package principal_module_can_link_to_the_actual_orchestrated_jobflow_heading -- --nocapture`. 합성 fixture에서 같은 정확한 위임 구조가 현재 실패하는지 먼저 검사한다. 함께 anchor 부재·다른 파일·다른 heading scope·도표 없음·inline code·절 밖 링크·중복 heading suffix 반례를 추가했다.
- 상태: 실행 직전. 이 담당자는 실제 모델을 추가 호출하지 않는다.

- D09 결과: 합성 fixture의 module 절이 실제 scoped orchestrator로 연결된 정상 입력이 **0 pass / 1 fail**, exit101로 실패했다. 오류는 `Method-R macro and module design need their jobflow DSL`였다. root가 관찰한 실제 응답 실패와 같은 주 문서 중복 강제임을 확인했다.

## PKG-D10 — 모듈 도표의 정확한 링크 위임

- 수정: principal macro scope 의무는 유지하고 module 판정은 package 경계에서 수행한다. 실제 heading ordinal·중복 suffix·계층을 보존하며 own module의 orchestrator 또는 module 절의 명시적 inline link가 가리키는 잡플로우 heading 영역의 orchestrator만 인정한다. 코드 span·다른 절·범위 없는 링크는 위임 근거가 아니다.
- 예정 명령: `rustfmt --edition 2024 src/document_format.rs src/design_package.rs tests/design_package.rs`, `cargo test --test design_package --test document_format --test domain_contract`, `cargo clippy --lib --test design_package -- -D warnings`. 상태: 실행 직전.

- D10 기능 결과: package **18 pass**·legacy 한 문서 **11 pass**·인터뷰 도메인 **13 pass**, 합계 **42 pass / 0 fail / 0 ignored**, exit0. 정상 연결, 동일 main 도표 경로, macro 필수, 중복 heading suffix와 가짜 링크·다른 영역 반례가 통과했다. lint 결과는 다음 항목에 별도 기록한다.

## PKG-D11 — 보존된 실제 Codex 응답의 무과금 replay

- 명령: `ARCHITALK_PACKAGE_RESPONSE_JSON=target/document-probes/3b881d77-4d6b-4109-8db1-29dacaa6b98f/response.json cargo test --test real_document_format real_codex_returns_a_method_r_design_package -- --ignored --nocapture`.
- 기준: 원본 response는 읽기만 하고 root가 추가한 replay 경로로 새 artifact/provenance를 남긴다. 환경변수를 명시해 `ai::generate` 경로를 실행하지 않는다. 여기서 새 실패가 드러나면 조건을 완화하지 않고 root에 오류와 근거를 먼저 전달한다. 상태: 실행 직전.

- D11 결과: **1 pass / 0 fail**, exit0. 보존 응답의 10문서 46,446 bytes가 통과했고 새 artifact는 `target/document-probes/02d8145a-5399-4daf-b8ce-4008b8b70e23/`이다. provenance의 `mode: retained-synthetic-response`, `model_calls: 0`을 별도 저장했다. 추가 모델 호출은 없다.

## PKG-D12 — 실제 link와 heading 경계에 대한 추가 반례

- root의 독립 읽기에서 단순 `](` 추출기가 bare closer·이미지·이스케이프된 링크도 위임으로 세는 반례를 받았다. scoped module 전용 링크 판정에 여는 대괄호·이미지·escape 및 destination 구문을 확인하는 테스트를 추가했다. 같은 heading 범위의 보장을 위해 실제 ATX의 0~3칸 들여쓰기 sibling 경계도 반례로 추가했다.
- 사전 명령: `cargo test --test design_package module_delegation_ -- --nocapture`. 상태: 실행 직전.

- D12 결과: **1 pass / 2 fail**, exit101. bare `](` 거짓 링크와 들여쓰기 sibling heading의 범위 이탈을 실제 validator에서 확인했다. root 반례를 무시하거나 정상 링크 요구를 완화하지 않는다.

## PKG-D13 — scoped module 전용 link·heading 경계 보완

- 수정: 공통 Markdown 링크 정책은 유지한다. 모듈 위임만 명시적 대괄호 label·여는 괄호·single-line destination을 확인하고 image/escape/bare closer를 배제한다. 범위 계수와 anchor 계수는 동일한 작은 ATX heading helper를 사용해 0~3칸 들여쓰기 및 빈 heading이 sibling 경계를 바꾸는 점을 보존한다. 일반 Markdown parser 추가·의존성 확대는 없다.
- 예정 명령: 관련 Rust 파일 rustfmt, `cargo test --test design_package --test document_format --test domain_contract`, `cargo clippy --lib --test design_package -- -D warnings`, D11과 같은 실제 보존 응답 replay. 상태: 실행 직전.

- D13 결과: package **20 pass**, legacy 한 문서 **11 pass**, 인터뷰 도메인 **13 pass**, 합계 **44 pass / 0 fail / 0 ignored**, exit0. rustfmt와 focused clippy exit0·경고0건. 정상 module 위임과 main 내 도표 모두 허용하면서 bare closer/image/escape/invalid title·indented sibling·다른 절·다른 heading·없는 anchor/도표는 거절했다.
- 최종 실제 replay: D11과 같은 원본 응답·환경변수 명령을 다시 실행하여 **1 pass / 0 fail**, 10문서 46,446 bytes 통과. 새 artifact `target/document-probes/a15680e0-4a40-4258-86b8-8afd1f4c5f1b/` 및 provenance `model_calls: 0`이다. 원본을 수정하지 않았고 추가 유료 호출은 없다.
- 소스 최종 freeze를 root에 통지했다. root의 추가 browser 4건·최종 전체 회귀·설치 검증은 해당 통합 이력에서 확인한다. 범위를 일반 CommonMark parser 구현으로 확대하지 않고, 모듈 위임에 필요한 한 줄 명시적 링크 subset과 실제 heading scope만 보장한다.
- 종료 확인: **2026-11-18 09:55:07 KST**, 소유 변경 대상의 `git diff --check -- src/document_format.rs src/design_package.rs tests/design_package.rs prompts/design-package-format.md tests/test-history/design-package-domain-20261118.md`도 exit0. 이 정적 검사를 동작 pass 수에 더하지 않는다.
