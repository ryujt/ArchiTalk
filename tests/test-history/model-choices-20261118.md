# 모델·생각 수준 선택과 직접 입력

- 기준: `main` / `8f723bf`, 시작 시 깨끗한 작업 트리. 모델·생각 수준의 기본/역할별 설정에 알려진 값 목록과 자유 입력을 함께 제공한다.
- 이전 이력: [브라우저 검증](implementation-web-browser.md) B05의 desktop/mobile 통과와 [프런트엔드 계약](implementation-web.md), [AI adapter](implementation-ai.md)의 전달/격리 검증을 참고한다.
- 담당: 통합 담당은 웹 UI·README·공용 색인, Backend Developer는 Rust 설정 metadata·입력 검증, 별도 Tester는 새 실제 브라우저 시나리오를 소유한다. 모델 설정은 상속하며 유료 모델 호출은 하지 않는다.
- 계약: 알려진 목록은 계정별 지원 보장이 아닌 예시다. 목록에서 선택하면 기존 입력값에 반영하고 직접 편집도 가능하다. 도구 변경 시 해당 목록만 갱신하고 입력값을 보존한다. 빈칸의 기본값 상속과 다음 작업부터 적용하는 동작은 유지한다.

## MC-UI-01 — 기존 웹 계약과 조립

- 상태: 2026-11-18 08:13:43 KST, `node --check web/app.js` 및 `node --test tests/web.test.mjs` 통과. Node 기존 21건 모두 통과·실패 0건(약 0.5초). 실제 브라우저와 Rust 조립 검증은 별도 진행한다.
- 방법: `node --check web/app.js`, `node --test tests/web.test.mjs`, 실제 UI 테스트 후 `cargo clippy --all-targets -- -D warnings`, 릴리스 설치.
- 판정: 기존 설정의 신뢰된 실행파일·profile 보존, 기존 UI 계약 회귀 없음, 최신 웹이 포함된 사용자 전역 바이너리 설치.
- 브라우저/Backend의 개별 실행·실패·재검증은 담당 기록에 연결한다. 이번 변경은 모델 지원 목록 조회를 위한 외부 API나 유료 모델 호출을 추가하지 않는다.

- [Backend 실행 이력](model-choices-backend-20261118.md)
- [실제 브라우저 실행 이력](model-choices-browser-20261118.md)

## MC-UI-02 — 정적 조립과 독립 검토

- 2026-11-18 08:14 KST `cargo clippy --all-targets -- -D warnings` 통과(약 2.5초, 최초 Cargo lock 대기 포함). Backend 변경과 새 테스트 target도 컴파일했다.
- 08:14:51 KST `cargo fmt --check`와 최종 diff 공백 검사 통과.
- 별도 Critic이 실제 diff를 읽기 전용 검토했다. 목록 select가 저장 payload에 섞이지 않는 점, GET choices의 비영속 metadata 분리, 도구 변경 시 입력값 보존, role 빈 값 상속, effort 토큰의 인자/TOML 주입 차단과 기존 권한 경계, label/설명 연결을 확인했고 필수 지적 없음으로 회신했다.

## MC-INSTALL-01 — 최신 릴리스 전역 설치

- 상태: 2026-11-18 08:15:24~08:15:40 KST 통과. `./install.sh`가 최신 릴리스를 빌드(약 6.5초)해 사용자 Cargo bin의 기존 실행 파일을 교체했다. `/tmp`의 새 `zsh -lc`에서 `command -v architalk`와 `architalk --version` 정상 실행을 확인했다. Backend의 AI 단위 12건·adapter 8건·설정 계약 2건 및 별도 Tester의 desktop/mobile 기존/신규 브라우저 4건 통과 보고를 확인했다.
- 방법: `./install.sh`, 다른 cwd의 새 `zsh -lc`에서 전역 명령과 버전 확인. 사용자에게 이미 설치한 전역 명령에 이번 내장 웹 화면과 backend 변경을 반영한다.
- 사용자 브라우저/기존 실행 프로세스는 종료하지 않는다. 새 실행부터 업데이트된 바이너리를 사용한다.

## 완료 판정

- 통합 담당은 UI·문서·설치를 맡고, Backend Developer는 선택 예시·검증·인자 전달을, 별도 Tester는 실제 브라우저 시나리오를, 별도 Critic은 최종 diff를 검토했다. 모델은 실제 상속 설정을 유지했다.
- 선택/직접 입력·역할별 목록 전환·값 보존·저장·복구·다음 실행 반영을 확인했다. 실제 모델의 계정별 이용 가능 여부는 이번 검증 범위가 아니며 기존 CLI의 오류 보고로 판단한다. 유료 모델 호출 0건.
