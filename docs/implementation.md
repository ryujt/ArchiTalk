# ArchiTalk 0.1 구현 안내

기준 설계는 [2026-11-17 TO-BE](design/20261117-190055/to-be/system-design-to-be.md)다. 설계 문서는 당시의 목표와 대안을 보존하고, 이 문서와 [README](../README.md)는 실제 제품 계약과 검증 범위를 설명한다.

## 실행과 데이터 경계

Rust 바이너리에 `web/` 정적 자산을 포함한다. 시작 시 cwd 또는 `--project`를 고정하고 root inode와 상태 파일을 잠근 뒤 loopback listener를 연다. API는 프로세스 토큰·정확한 Host/Origin으로 보호한다. 브라우저는 URL fragment 토큰을 제거하고 같은 origin의 fetch HTTP/SSE를 사용한다.

`App`은 하나의 논리 writer이며, 짧은 SQLite transaction에 workspace snapshot·receipt·event를 함께 저장한다. AI를 기다리는 동안 DB lock을 유지하지 않는다. 모델 프로세스와 help 진단은 같은 프로젝트 실행 slot을 선예약한다. 생성 중 설정 변경은 다음 run에 적용한다.

프로세스 결과는 terminal event·exit·출력 스키마·참조를 모두 확인한 뒤 반영한다. 취소·오류·태스크 panic은 완료와 구분하고 종료 미확인 상태는 재시작 후에도 보존한다. 설계 문서가 만들어진 뒤 비판 검토 준비가 실패하면 초안과 설계 완료 결과는 보존하고 검토 실패를 따로 기록한다.

## 공개 API

경로 앞에는 `/api/v1`을 붙인다. 변경 요청은 `{command_id, expected_revision, payload}`를 사용한다. 동일 command ID의 receipt를 revision 검사보다 먼저 확인하며, 다른 payload로 재사용하면 거부한다. `revision`은 명령 동시성, `design_revision`은 의미 입력 버전이다.

| 경로 | 계약 |
| --- | --- |
| `GET /workspace` | 프로젝트·세션·run·export·선택 가능한 자료·현재 cursor |
| `POST /sessions` | `title`, `intro`, `context_paths`로 생성하고 대화 run 접수 |
| `GET /sessions/{id}` | 세션과 현재 초안의 readiness 문제·cursor |
| `POST /sessions/{id}/messages` | `content` 저장과 대화 run |
| `GET/POST /sessions/{id}/draft` | 완성 초안 조회 / 작성 run 접수 후 독립 비판 검토 |
| `POST /sessions/{id}/critic` | 마지막 완성 초안의 명시적 재검토 |
| `POST /sessions/{id}/review` | 승인·지적 처리·진술 확인·범위 보류 |
| `GET /runs/{id}` | 실제 완료/실패/중단/종료 확인 상태 |
| `POST /runs/{id}/cancel` | 취소 접수. OS 종료 확인 전 slot 유지 |
| `POST /runs/{id}/recover` | 직접 프로세스를 정리한 뒤 `{confirmed:true}`로 확인. 세션 없는 진단은 workspace revision 사용 |
| `GET /sessions/{id}/events` | durable event 재생과 생성 progress/delta |
| `GET /runs/{id}/events` | 해당 run의 progress/delta와 세션 상태 변경 |
| `POST /sessions/{id}/exports` | preview: `draft_id`, 선택적 `directory`; apply: `plan_id` |
| `GET /sessions/{id}/exports/{id}` | 정확한 파일별 저장 결과 확인 |
| `GET/PATCH /settings` | 설정과 알려진 모델·생각 수준 예시(`choices`) 조회 / `payload.settings` 전체 갱신 |
| `POST /settings/diagnostics` | 모델 호출 없는 설치·버전·필수 옵션 진단 |

`202`는 접수이며 생성 완료가 아니다. SSE delta는 임시 데이터이고 마지막 확정 snapshot이 기준이다. feed를 놓치면 cursor replay 또는 snapshot polling으로 복구한다. UI는 불명확한 저장 응답을 같은 export ID로 조회해 확인하고 새로운 저장을 겹치지 않는다.

## 대화·검토·저장

- 기본 coverage와 결제/민감 데이터/되돌릴 수 없는 변경 overlay를 적용한다. 범위 크기와 실패 영향을 분리하고, 중요한 미정/위험 질문을 코드가 우선한다. v1 pack은 내장 정책이며 외부 실행 플러그인은 없다.
- AI가 추출한 내용은 해석/가정/제안으로 남긴다. 근거 ID와 원문 인용을 확인하며, 사용자 사실로 자동 승격하지 않는다. 최신 답변이 모름·추천·보류이면 이를 잘라 인용해 확정 수정의 근거로 사용할 수 없다.
- 요구·진술·결정 수정은 새 버전과 `supersedes` 연결을 만들고 이전 근거를 보존한다. 과거 값을 익명 신규 항목으로 부활시키지 않는다. 연결된 결정은 변경 영향을 재검토한다.
- 내용 승인과 파일 저장을 분리한다. 근거가 있는 활성 요구, 필수 coverage, 독립 검토, 주요 지적 처리, 현재 설계 버전이 검토본 확정의 조건이다. 주요 지적을 다시 열면 기존 승인도 철회한다.
- 초안은 미생성/현재/이전 버전으로 구분한다. 과거 초안은 해당 버전으로 저장할 수 있지만 현재 설계의 승인으로 승격하지 않는다.
- preview는 source draft hash·설계 버전·현재 세션 revision에 연결한다. 이후 대화나 검토가 바뀌면 새 preview를 요구한다. 출력에는 근거와 미정, 지적 처리와 내용 상태를 포함한다.
- Store에서 파생한 `ProjectFiles`는 원래 root 디렉터리 handle을 유지한다. 경로를 symlink로 바꿔도 외부로 쓰지 않는다. 새 bundle을 배타적으로 생성하고 파일은 no-clobber publish, manifest는 마지막에 반영한다. 기존 문서를 덮어쓰지 않는다.

## 설계 대비 v1 범위

| 항목 | 실제 구현 |
| --- | --- |
| 로컬 Rust CLI + 웹 대화 | 포함된 HTML/CSS/JS, Node 실행 의존성 없음 |
| Claude/Codex·모델·생각 수준 | 역할별 선택·고정 run 설정, 실제 CLI stdout 프로토콜 해석 |
| CLI 버전/권한 | 검증한 정확한 버전 프로필, macOS 프로젝트 쓰기 차단. 다른 OS 빌드·named profile은 미지원; 미검증 실행 조건은 거부 |
| 설정 계층 | 사용자 TOML + 명시적 프로젝트 웹 설정(SQLite). 저장소의 임의 실행파일/args/env는 로드하지 않음 |
| 문서 분할 | 기본 한 Markdown 문서와 manifest. 외부 pack/template 및 자동 다중 명세 분할은 확장 지점 |
| 문서 표시 | 안전한 plain text Markdown. Mermaid/HTML 실행·렌더링 엔진은 포함하지 않음 |
| 진단 보존 | stderr의 단계·오류 요약. 파일 logger와 원문 대화 덤프는 기본 비활성 |
| 운영 | CLI/브라우저 재시작·파일 대사·수동 종료 확인. 강제 부모 종료/전원 손실의 자동 고아 정리 보장은 없음 |

이 범위는 초기 사용자 요구의 실행·대화·설계 문서·로컬 AI 설정 경로를 구현한다. 모델 답변의 의미적 정답, 모든 분야의 규칙, 모든 CLI/OS 버전 호환을 기계 검사로 보장하지 않는다.

## 검증 근거

[실행 이력 색인](../tests/test-history/README.md)에 최초 실패와 수정·재검증을 보존한다. 소스 코드 단위/대역과 로컬 실물·실제 모델·브라우저 증거를 구분한다. 실제 실행하지 않은 Linux/Windows·전원 손실·장기 부하·실제 disk-full 결과는 통과로 표시하지 않는다.
