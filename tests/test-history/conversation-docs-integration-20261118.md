# 대화 화면·SmartMD 문서 통합 검증

- 기준: `main` / `5954230`의 작업 트리. 사용자가 연속 요청한 Codex 진행 안내, 대화 완료 스크롤, 다음 단계 CTA, 인라인 작성 표시, 넓은 대화 영역, SmartMD 설계문서를 통합한다.
- 이전 이력: [Codex 안내](codex-progress-20261118.md) CP-01, [UI 구현](chat-scroll-implementation-20261118.md), [실제 UI](chat-scroll-browser-20261118.md) B01 수정 전 실패/B02 수정 후 8건 통과, [문서 생성](smartmd-documents-20261118.md), [mdview 호환](smartmd-renderer-20261118.md)을 연결한다.
- 소유: root는 공유 fixture 복사·문서 안내·색인·최종 조립/설치. Developer는 UI, 다른 Developer는 문서 생성/구조 검증. 별도 Tester는 실제 브라우저 및 원본 mdview 변환기를 확인했다/확인한다. 코드 작성자와 다른 관점의 검토를 적용하며 상속 모델을 유지한다.
- 통합 변경: Python fixture의 새 설계 응답은 `architecture.smart.md`를 읽고, Rust/browser 테스트 helper는 같은 파일을 자신이 만든 임시 디렉터리에 함께 복사한다. 기존 알림 서비스 제목과 외부 모델 미호출 경계를 유지한다.

## INT-01 — Rust·구문·정적 조립

- 상태: 2026-11-18 08:36:49 KST 시작, Python fixture AST·`cargo fmt --check`·`cargo clippy --all-targets -- -D warnings`·`cargo test --locked` 모두 통과. Rust 80건 통과·실패 0건, 실제 모델 4건은 기본 실행에서 ignored. 형식 검사 11건과 새 문서 fixture의 실제 subprocess→DB→파일 workflow, 과거 문서 export 호환도 포함한다.
- 명령: Python fixture AST 구문 확인, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`.
- 판정: 새 문서 구조/비적용 예외/잘못된 출력 거절, 기존 인터뷰·저장·권한·프로세스 회귀 유지, 과거 문서 내보내기 호환. 실제 모델 테스트는 ignored 유지.
- 브라우저는 새 출력 형식과 fixture가 바뀐 영향 때문에 최종 8건을 재검증한다. 앞선 B02의 결과를 현재 문서계약의 통과로 대체하지 않는다.
- 최종 B03: 08:37:22~08:37:48 KST, 새 SmartMD 파일 저장 assertion을 포함한 실제 desktop/mobile 8건 통과(23.2초). 세부 PNG 시각 확인과 결과는 연결한 브라우저 이력을 따른다.
- root 독립 검토에서 mdview block parser의 열0 fence 기준과 새 검사기의 들여쓰기 허용 차이를 지적했다. Developer가 열0 전용 DSL 경계와 반례를 반영했다. 작성자와 분리한 Critic의 가짜 표 사양/중첩 예제 등 반례도 반영해 11건으로 재검증했다.

## INT-02 — 최종 사용자 전역 설치

- 상태: 2026-11-18 08:39:19~08:40:06 KST 통과. `./install.sh`의 release 빌드(약 6.8초)와 전역 실행 파일 교체 성공. `/tmp`의 새 `zsh -lc`에서 `~/.cargo/bin/architalk` 해석과 `architalk 0.1.0` 실행을 확인했다.
- 사용자가 실행한 서버/브라우저를 종료하지 않는다. 새 실행부터 최신 내장 UI/생성 규칙을 사용한다. 유료 모델 호출은 하지 않는다.

## INT-REAL-01 — 새 생성 형식의 실제 Codex 1회 확인

- 상태: 형식 Developer 완료·기본 Rust 검사 통과 후 실행 준비. 위 기본/fixture 검증의 모델 미호출 범위와 별도다.
- 근거: 새 프롬프트가 실제 모델에서 4개 DSL과 SmartMD로 응답하는 경계를 확인한다. 합성 목록/상세 웹 도구만 전달한다.
- 명령: `cargo test --test real_document_format -- --ignored --nocapture`. 기본 전체 suite에서는 ignored이며, 이 명시 실행만 Codex 사용량이 발생한다.
- 정확히 한 번 `ai::generate`를 호출하고 자동 수리·재시도하지 않는다. 모델 subprocess는 기존 OS 격리를 사용한다. 결과는 Git 제외 `target/document-probes/<uuid>/`에 합성 응답/문서를 보존한다. 인증·사용자 프로젝트·접속 토큰은 기록하지 않는다.
- 결과: 2026-11-18 08:37:19 KST 시작, 실제 Codex 설계 1회가 약 59.6초 후 완료했다. `validate_design`과 4종 DSL 포함 검사 통과, 10,693바이트 문서. 합성 근거: `target/document-probes/365fcc84-3bc8-4134-adfa-fde439fa92de/`. 자동 추가 호출은 없었다. 이 모델 호출 1건은 위 기본/브라우저 테스트의 모델 미호출 범위와 구분한다.
- 별도 Tester가 이 실제 생성물의 navigation 2개·state 2개·jobflow 1개·layout 2개를 원본 mdview API로 변환하고 SmartMD 전처리를 확인했다. 예외·빈 결과·비정상 수치 없음. 이 결과는 문자열 변환/구조 호환성 증거이며 전체 mdview 화면/Mermaid SVG 시각 검증은 아니다. 세부 기록은 renderer 이력 R04를 따른다.

## 완료

- 필수 검증 통과. 원본 guide/mdview 저장소와 사용자 실행 중인 서버·브라우저는 변경하지 않았다. 기존 문서와 모델 설정은 보존한다.
- 최종 제품 코드는 UI/문서 Developer와 분리한 Critic, 실제 브라우저/renderer Tester, 통합 담당자의 검토를 거쳤다. 테스트 실패와 수정 전 스크롤 재현은 연결 이력에 유지한다.
