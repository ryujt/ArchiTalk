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
| Codex 일반 진행 안내 | [codex-progress-20261118](codex-progress-20261118.md) | 진행/오류 판정 기존 계약 통과, 한국어 안내 반영 |
| 대화 스크롤·인라인 작성·다음 단계·넓은 화면 구현 | [chat-scroll-implementation-20261118](chat-scroll-implementation-20261118.md) | Node 21건·독립 코드 검토 통과 |
| 실제 긴 대화 스크롤·작성 중 화면·넓은 화면 | [chat-scroll-browser-20261118](chat-scroll-browser-20261118.md) | 수정 전 2실패 보존, 수정 후 desktop/mobile 전체 8건 통과 |
| SmartMD 문서 생성·형식 계약 | [smartmd-documents-20261118](smartmd-documents-20261118.md) | focused 11건·독립 반례 검토 통과 |
| 원본 mdview 변환기와 실제 문서 호환 | [smartmd-renderer-20261118](smartmd-renderer-20261118.md) | 대표 문서 6건·실제 Codex 문서의 7블록 변환 통과 |
| 대화 화면·문서 생성 최종 조립·전역 설치 | [conversation-docs-integration-20261118](conversation-docs-integration-20261118.md) | Rust 80건·브라우저 8건·실제 Codex 설계 1회·전역 설치 통과 |
| 앱 내 Markdown·SmartMD renderer | [document-renderer](document-renderer-20261118.md) | 실제 renderer·정화·소스 검토 통과 |
| 앱 렌더링·도표·적대적 문서 브라우저 | [document-rendering-browser](document-rendering-browser-20261118.md) | 원문 표시 재현·수정, 최종 12건 통과 |
| renderer HTTP·자산·앱 조립 | [document-rendering-integration](document-rendering-integration-20261118.md) | Node 21·Rust 80·clippy·브라우저 통과 |
| Method-R 패키지 생성·품질·추적성 계약 | [design-package-domain](design-package-domain-20261118.md) | 도메인 44건 통과, HTML 위장·모듈 링크 범위 반례 수정 |
| 10파일 발행·WAL 백업·schema2 호환 | [design-package-storage](design-package-storage-20261118.md) | 집중 37건·실제 구버전 binary 거절 1건 통과 |
| 문서 종류 선택·패키지 링크·anchor | [design-package-ui](design-package-ui-20261118.md) | 기존·신규 Node 28건 통과, 시간 제한 설정 포함 |
| 10종 문서·anchor·전체 파일 저장 브라우저 | [design-package-browser](design-package-browser-20261118.md) | desktop/mobile 전체 14건·실제 원문/시간 제한 추가 4건 통과, 실패 이력 보존 |
| 종류별 설계·전체 검토·저장 최종 조립 | [design-package-integration](design-package-integration-20261118.md) | Rust 108·Node 28·브라우저 전체14+추가4 통과, 실제 응답·전역0.2.0 확인 |
| 프로젝트 기록 삭제·공통 AI 설정·실제 CLI 조립 | [project-state-settings](project-state-settings-20261118.md) | Rust129·Node33·browser16+후속2·실제 Claude327/Codex tiny 요청 통과 |
| 삭제·교체 감지·SSE·프로세스 취소·락 소유 종료 | [project-state-reset](project-state-reset-20261118.md) | 삭제 반례·fork 상속 재현 후 집중47 통과 |
| CLI 버전·project runtime·Codex home/auth 읽기 | [project-state-ai](project-state-ai-20261118.md) | 327 명시 지원, private-home 대조·원인 수정·제한/진단 검증 |
| 이전 화면 제거·새 연결·same-origin 재시작 | [project-state-ui](project-state-ui-20261118.md) | 최초 반례4실패 보존, Node27·browser16+후속2 통과 |

최초 작업으로 참고할 이전 실행은 없었다. 후속 실행은 각 기록의 미검증·후속 조건을 먼저 확인한다.
