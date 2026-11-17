# 검증 실행 이력

제품 구현 테스트와 초기 설계 문서 검증을 구분한다. 각 실행의 실제 환경·실패·수정·재검증·미실행 범위는 개별 기록에 보존한다.

| 대상 | 실행 기록 | 상태 |
| --- | --- | --- |
| TO-BE 본문·흐름·참조·통합 검토 | [20261117-tobe-validation](20261117-tobe-validation.md) | 독립 검토 반영·정적 통과, 브라우저 차단 |
| UI 목록·DSL·JS·장면 생성·예시 동작 | [ui-20261117-static](ui-20261117-static.md) | S01/S02 통과, 시각 QA 미검증 |
| 앱 조립·HTTP·실제 CLI·저장·재시작 | [implementation-integration](implementation-integration.md) | 전체 65건+추가 AI 회귀, 실제 Claude/Codex 전체 경로 2건 통과 |
| AI adapter·프로세스·격리 | [implementation-ai](implementation-ai.md) | 오류 경로 panic 재현·수정, 단위/통합·실제 CLI smoke 기록 |
| SQLite·경로·문서·복구 | [implementation-storage](implementation-storage.md) | 26건 및 대형 snapshot 검증, 독립 반례 수정 |
| 근거·수정 이력·질문 정책 | [implementation-domain](implementation-domain.md) | 13개 반례 회귀 통과·독립 검토 반영 |
| 실제 UI 코드·SSE·상태 | [implementation-web](implementation-web.md) | 초기 Node 19건, 후속 경합 회귀는 브라우저 기록에 연결 |
| 실제 desktop/mobile UI·모델 설정·저장·복구 | [implementation-web-browser](implementation-web-browser.md) | Node 21건·실제 headless Chrome 2건 통과, 실패/수정 보존 |
| 전역 CLI 설치 스크립트·PATH | [install-20261118](install-20261118.md) | 상대 prefix·기존 PATH 안내·기본 전역 설치 통과, 독립 검토 반영 |
| 모델·생각 수준 선택 및 직접 입력 조립 | [model-choices-20261118](model-choices-20261118.md) | Node 21건·clippy·format·최신 전역 설치 통과, 독립 검토 완료 |
| 모델 선택 metadata·안전한 effort 전달 | [model-choices-backend-20261118](model-choices-backend-20261118.md) | 단위 12건·adapter 8건·HTTP/진단 2건 통과, 실제 CLI 1건 ignored |
| 실제 모델/effort 선택·직접 입력 UI | [model-choices-browser-20261118](model-choices-browser-20261118.md) | desktop/mobile 기존·신규 4건 통과 |

최초 작업으로 참고할 이전 실행은 없었다. 후속 실행은 각 기록의 미검증·후속 조건을 먼저 확인한다.
