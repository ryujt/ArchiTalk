# 모델·생각 수준 목록/직접 입력 브라우저 검증

## 사전 범위 — 2026-11-18 08:13 KST

- 기준: `8f723bf`의 작업 트리. 제품 UI와 backend 목록 계약은 병렬 구현 중이며 이 문서는 브라우저 실행 전에 작성했다.
- 목표: Claude/Codex 및 대화·설계·비판 역할에서 목록 선택과 자유 입력을 함께 사용하고, 설정 저장·재열기·새로고침 후 다음 fixture Run에 적용되는지 확인한다.
- 쓰기 소유권: `tests/browser/settings.spec.mjs`, 이 기록. 공용 fixture, 제품 소스, 기존 테스트는 수정하지 않는다. 색인 병합은 root 담당자에게 전달한다.
- 이전 기록: [implementation-web-browser](implementation-web-browser.md) B04의 snapshot 경합 수정과 B05 desktop/mobile 통과를 참고했다. 기존 워크플로 전체를 다시 작성하지 않고 이번 설정 계약을 새 실제 UI 시나리오로 검증한다. 기존 브라우저 fixture의 격리·Bearer·외부 요청 차단·토큰 비기록·7일 artifact 보존을 그대로 사용한다.
- 환경: 저장소가 시작하는 headless Chrome, desktop 1365×900 / mobile 390×844. 각 테스트의 새 임시 프로젝트·설정·loopback 서버·브라우저 context. 모든 subprocess 실행 파일은 `tests/fixtures/claude-workflow.py` 복사본이다. 사용자 브라우저나 실제 모델에는 연결하지 않는다.

| 시나리오 | 판정 근거 |
| --- | --- |
| 진단 전에 알려진 목록 표시 | 실제 GET `/api/v1/settings`의 `choices`, 10개 select의 option value와 10개 기존 input 편집 가능, diagnostics 요청 0건 |
| 목록 선택과 직접 입력 | 각 backend/역할의 목록 선택→input 반영, custom 입력→목록 빈 값, 알려진 값 직접 입력→목록 일치, 직접 입력 옵션 선택 시 값 보존 |
| backend 변경 | 기본 도구와 역할 도구를 바꾸면 effective backend 목록만 바뀌며 모든 model/effort 입력 값 보존 |
| 저장·복구·상속 | 실제 workspace 설정의 known/custom/null 값, 재열기와 reload 후 UI 값·목록 선택 보존, 설정만으로 Run 생성 없음 |
| 다음 실행 | Claude fixture의 conversation/design/critic 완료와 각 Run의 effective model/effort, 빈 역할 값의 backend 기본값 상속 |
| 실제 화면 | desktop/mobile 설정 상단·하단 screenshot과 가로 넘침 없음, pageerror·외부 요청·Bearer 위반 없음 |

계획 명령: 제품 담당자가 준비를 알린 후 `ARCHITALK_BROWSER_RUN=<고유 run ID> npm run test:browser`. 설정 모달의 DOM·초점 수가 바뀌므로 새 설정 시나리오와 기존 `workspace.spec.mjs`를 함께 실행한다. 추가 검사는 작성 파일 구문·공백·정적 diff 검토로 한정한다.

## 실행 기록

### S01 — 새 테스트 구문 / diff 공백

- 시각: 2026-11-18 08:14~08:15 KST 범위, 개별 명령 벽시계 시작·종료 미측정. 기준: 위 작업 트리, backend/UI 구현 완료 통지를 받은 상태.
- 명령: `node --check tests/browser/settings.spec.mjs`, `git diff --check`.
- 결과: 둘 다 exit 0. 저장소 전체 diff 공백 검사는 공유 작업 트리의 변경도 포함했다.

### B01 — 전체 desktop/mobile 브라우저 회귀

- 시각: 2026-11-18 08:14~08:15 KST 범위에서 실행, 종료 출력 관찰 **08:15:08 KST**. 개별 명령 시작 시각은 미측정. 사전 지정 run ID: `run-1794957300000`.
- 명령: `ARCHITALK_BROWSER_RUN=run-1794957300000 npm run test:browser`.
- 범위: 새 설정 시나리오 desktop/mobile 2건 + 기존 workspace 시나리오 desktop/mobile 2건. pre script의 `cargo build --bin architalk`로 embedded UI를 최신화한다.
- 결과: **4건 통과·0건 실패**, exit 0. cargo build 약 **3.18초**, Playwright **11.7초**. 한 번 실행했고 재시도·부분 재실행은 없었다. 실행 중 npm/Node의 `NO_COLOR`와 `FORCE_COLOR` 동시 지정 경고가 있었으나 테스트 실패·제품 pageerror는 없었다.
- 새 시나리오의 실제 확인: GET `choices` 정확한 값, 진단 없이 10개 목록 표시와 기존 input 편집 가능, 모든 backend/역할의 목록→input 및 input→목록 동기화, custom 값에서 직접 입력 옵션 선택 시 유지, 기본/역할 backend를 전환해도 input 보존 및 effective backend 목록 갱신, known/custom/빈칸의 저장·재열기·reload 후 유지, 빈칸의 JSON null 상속, 설정 조작으로 Run 생성 없음, 다음 fixture conversation의 `sonnet/high` 상속·design의 `fixture-custom-design/medium` 재정의·critic의 `opus/max` 선택 적용. fixture subprocess 3건 모두 완료했고 settings snapshot이 유지됐다.
- 기존 workspace 시나리오도 함께 통과: 초안→실제 파일 저장, 설정 다음 실행 적용, 기존 Run 스냅샷 불변, reload 복구, native modal 초점·background inert·Escape 복귀, mobile/tablet 화면 확인. 테스트별 pageerror·동일 origin/Bearer 위반·외부 요청 0건.
- 캡처: `target/browser-artifacts/run-1794957300000/`. 새 설정 desktop/mobile 각 3장 + 기존 workspace 11장 = **17장**. 기존 7일 artifact retention을 따르며 Git에는 포함하지 않는다. 토큰·헤더·trace/HAR/video는 기록하지 않았다.

### R01 — 정적 diff 및 실제 screenshot 검토

- 시각: 2026-11-18 08:15 KST, B01 완료 후. 검토자는 제품 작성자와 분리된 브라우저 검증 담당자다.
- 읽은 diff: `web/app.js`, `web/style.css`, `src/ai.rs`, `src/app.rs`, 관련 adapter/API 계약 테스트. settings GET에 목록 추가, 원래 input ID/저장 필드 유지, 선택 목록의 HTML escape, backend 변경 시 input DOM/값 보존, choices 미제공 시 직접 입력 fallback, effort custom token 검증과 CLI 인자 전달을 대조했다. 요청 범위에서 필수 잔존 결함은 발견하지 못했다.
- 실제 열어 본 PNG: 새 설정 desktop/mobile의 `01-settings-choices.png`, `02-role-choices.png` 총 4장. Claude 목록/입력 조합과 역할 목록/입력·적용 버튼, 모바일 한 열 배치·줄바꿈·모달 스크롤을 확인했다. 설정 모달과 문서의 가로 넘침 assertion도 통과했다. 전체 스크롤 영역이 한 장에 모두 담긴다고 주장하지 않으며 상단/하단을 따로 캡처했다.
- 캡처 하위 폴더: `settings-known-model-choic-cd8c1-d-and-the-next-fixture-runs-desktop/`, `settings-known-model-choic-cd8c1-d-and-the-next-fixture-runs-mobile/`.

## 한계와 다음 실행

- 실제 Claude/Codex 계정의 모델 접근 권한·응답 품질·요금은 검증하지 않았다. 알려진 값은 목록 예시이며 CLI/모델 지원 확인으로 표시하지 않는다.
- 이 브라우저 fixture는 Claude stream 형식이므로 실제 next Run 검증은 Claude backend에서 수행했다. Codex custom model/effort는 UI 저장·reload까지 확인했으며 실제 Codex 인자 전달은 병렬 backend 계약/adapter 테스트 기록을 참조한다.
- Chrome 한 버전의 desktop/mobile 및 기존 tablet 표본이다. 이번 요청에서 바뀐 설정 흐름 외의 전수 감사는 하지 않았다. 발견한 제품 결함·미종결 실행은 없다.
