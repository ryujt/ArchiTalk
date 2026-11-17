# 생성 설계 문서의 SmartMD 계약 검증

## SM-01 — 구현·검증 준비

2026-11-18 KST, 실행 전에 기록했다. 기존 `README.md` 이력 색인과 `implementation-domain.md`, workflow fixture 및 생성/critic/export 호출을 읽었다. 현재 `validate_design`은 200바이트 이상·제목 문자만 검사하므로 전용 DSL과 SmartMD가 누락돼도 통과한다. 생성된 초안에 trace appendix가 붙고 저장/내보내기는 기존 문자열을 보존한다.

원본 project-guides의 Navigation 예제 선택 목록 → 작성 샘플·작은 완성 예제, navigation/jobflow/state/layout 전용 가이드 및 SmartMD 문법을 확인했다. mdview 실제 JS 구현의 허용 문법을 대조했다. 두 원본 저장소는 수정하지 않는다. 내장 규칙은 생성과 critic가 공유하며, 이번 구조 검사는 새 생성 응답에만 적용한다. 과거 저장 문서의 표시·내보내기를 차단하지 않는다. 비UI의 navigation/layout은 명시한 이유와 대체 CLI/API 인터페이스가 있는 경우에만 예외를 허용한다.

계획: 대표 단일 Markdown fixture, 누락·가짜 펜스·잘못된 DSL·비UI 예외·과거 export 보존·프롬프트 계약에 대한 focused Rust 검증. 실제 mdview parser 확인은 root 소유의 독립 검증으로 구분한다. 유료 모델 호출·설치·커밋은 하지 않는다.

## SM-02 — 생성 계약과 새 응답 검사

생성과 critic에 같은 `include_str!` 규칙을 제공했다. 단일 문서의 첫 줄 frontmatter, 실제 callout·SmartMD 열 사양+GFM 표, 고정 H2와 해당 전용 DSL을 검사한다. jobflow 객체/관계, 상태 노드, layout 연결 트리·중복·순환도 기본적으로 확인한다. Navigation/Layout 비적용은 각 섹션에서 명시적 `Applicability: not_applicable`, 이유, 대체 인터페이스를 함께 적어야 한다. 단일 페이지에서 이동이 없는 과업도 이유와 실제 Layout 연결로 설명할 수 있다. 도메인 의미와 비적용의 사실 여부는 자료를 받은 critic가 검토하며 문자열 검사로 증명하지 않는다.

새 응답은 LF로 정규화하고 `architalk-v3-smartmd` prompt metadata를 기록한다(JSON schema_version은 2 유지). 기존 저장 초안의 읽기·내보내기에는 검사를 소급하지 않는다. `App::execute`는 design 검증 실패에 자동 repair 모델 호출을 하지 않으며 실패 상태를 보존한다. conversation/critic의 기존 schema repair 정책은 유지한다.

1차 `cargo test --test document_format -- --nocapture`: exit 0, 10/10 통과, 테스트 실행 0.59초. 유료 모델 대신 외부 임시 Python CLI stand-in을 썼다. 실제 App→subprocess→SQLite 경로에서 invalid design이 정확히 1회 생성되고 실패하며 새 초안이 생기지 않는 경우와 기존 초안이 정확히 보존되는 경우를 모두 확인했다.

## SM-03 — 독립 반례와 최종 focused 검증

하위 Critic가 발견한 `{table caption="title=title=" columns=[]}` 허위 열 사양, mdview 특유의 중첩 text fence 경계, 코드펜스 안 HTML 주석 리터럴 반례를 수용했다. 열 사양은 인용 문자열 밖의 실제 column 객체를 읽고, 중첩 text 예제는 mdview의 경계 규칙으로 가리며, 코드 리터럴의 주석은 원문으로 보존한다. root 요청으로 전용 DSL의 opening/closing fence는 열0이고 언어가 마커 바로 다음에 오도록 제한했다. 허용한 보수적 노드 문자 제한도 프롬프트에 명시했다.

최종 `cargo test --test document_format -- --nocapture`: exit 0, **11/11 통과**, 테스트 실행 0.47초. `cargo clippy --test document_format -- -D warnings`: exit 0. 소유 Rust 파일 rustfmt 및 `git diff --check` 통과. 최종 작업 시계 관측은 **2026-11-18 08:35:50 KST**였다. 처음 두 테스트 실행의 정확한 초 단위 시작시각은 캡처하지 않아 추정하지 않는다.

독립 Critic는 수정과 검사를 다시 읽고 필수 잔존 지적 없음을 확인했다(동적 검사는 중복 실행하지 않았다). 별도 Tester의 실제 mdview API 검증은 `smartmd-renderer-20261118.md`에 기록한다. 이 작업자는 유료 모델 생성, Mermaid SVG 화면 표시 또는 전체 뷰어 시각 QA를 실행하지 않았다. 전체 Rust/browser 조립검증은 root 소유다. 원본 두 저장소 수정·설치·커밋 없음.
