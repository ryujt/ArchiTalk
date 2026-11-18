# ArchiTalk 0.6 구현 안내

기준 설계는 [2026-11-17 TO-BE](design/20261117-190055/to-be/system-design-to-be.md)다. 설계 문서는 당시의 목표와 대안을 보존하고, 이 문서와 [README](../README.md)는 실제 제품 계약과 검증 범위를 설명한다.

## 실행과 데이터 경계

Rust 바이너리에 `web/` 정적 자산을 포함한다. 시작 시 cwd 또는 `--project`를 고정하고 root inode와 상태 파일을 잠근 뒤 loopback listener를 연다. API는 프로세스 토큰·정확한 Host/Origin으로 보호한다. 브라우저는 URL fragment 토큰을 제거하고 같은 origin의 fetch HTTP/SSE를 사용한다.

페이지 아이콘도 바이너리에 포함하고 `/favicon.ico`에서 제공한다. 정적 자산에는 API 토큰이 필요 없지만 동일한 Host/Origin 검사와 보안 헤더를 적용한다.

`App`은 하나의 논리 writer이며, 짧은 SQLite transaction에 workspace snapshot·receipt·event를 함께 저장한다. AI를 기다리는 동안 DB lock을 유지하지 않는다. 모델 프로세스와 help 진단은 같은 프로젝트 실행 slot을 선예약한다. 생성 중 설정 변경은 다음 run에 적용한다. 제품 main은 공통 설정 파일을 App에 연결하고 AI 설정을 매 작업 전에 다시 읽는다. 웹 저장은 파일 lock·내용 hash 비교·atomic rename으로 AI 일반 설정만 갱신한다. 문서 경로·대화·run은 프로젝트 DB에 남는다. 테스트의 `App::open`은 공통 파일을 건드리지 않는 격리 모드다.

AI 요청의 기본 시간 제한은 1800초(30분)이며 웹/TOML 설정의 `ai.timeout_seconds`로 10..3600초를 지정한다. 명시한 기존 TOML 값은 유지하고 파일/필드가 없을 때 새 기본값을 사용한다. 대화·설계·검토의 각 작업 시작 시 설정을 고정하며 같은 작업의 형식 보정도 그 값을 유지한다. 생각·출력 생성 중 진행 이벤트가 계속 들어와도 deadline은 연장하지 않는다. 모델 실행 전 진단과 종료 정리 시간은 별도이며, 대화/검토의 형식 보정도 별도 요청이다.

DB schema는 3이다. v1/v2에서 이행하기 전에 SQLite online backup으로 WAL의 확정 내용까지 포함한 별도 DB를 만들고, schema 번호를 갱신한다. 이전 실행 파일은 더 새로운 schema를 거부해 생성 진행 기록의 유실을 막는다. 과거 snapshot의 `Draft.documents`·`Session.design_generations` 누락은 빈 목록으로 읽고 기존 단일 문서 hash와 발행 복구를 유지한다.

프로젝트 root·`.architalk`·DB의 실제 파일 identity가 실행 중 바뀌면 `workspace_reset_required`로 이전 workspace 제공과 저장을 차단한다. old inode의 메모리/SQLite를 새 폴더에 복사하지 않으며 진행 중 프로세스를 중지한다. HTTP 410/SSE 알림을 받은 브라우저도 대화·문서 캐시를 비우고 서버 재시작을 안내한다. 새 서버는 보이는 새 폴더만 읽는다.

프로세스 결과는 terminal event·exit·출력 스키마·참조를 모두 확인한 뒤 반영한다. 취소·오류·태스크 panic은 완료와 구분하고 종료 미확인 상태는 재시작 후에도 보존한다. 설계 문서가 만들어진 뒤 비판 검토 준비가 실패하면 초안과 설계 완료 결과는 보존하고 검토 실패를 따로 기록한다.

버전/help 진단부터 프로젝트 runtime에서 쓰기 제한을 적용한다. Claude는 2.1.326/2.1.327 명시 프로필과 필수 제한 옵션을 함께 확인한다. Codex는 native HOME도 run-local로 두고 공용 파일 인증을 metadata 검증 뒤 읽기 symlink로 연결한다. 공용 인증의 복사·쓰기는 허용하지 않으며 키체인 전용 Codex 인증은 지원하지 않는다. 실패 진단은 고정 reason/component 분류만 남겨 stderr의 원문·토큰·프롬프트를 노출하지 않는다.

## 공개 API

경로 앞에는 `/api/v1`을 붙인다. 프로젝트 변경 요청은 `{command_id, expected_revision, payload}`를 사용한다. 동일 command ID의 receipt를 revision 검사보다 먼저 확인하며, 다른 payload로 재사용하면 거부한다. `revision`은 명령 동시성, `design_revision`은 의미 입력 버전이다. 별도의 전사 설정·전사 요청은 아래의 직접 JSON/multipart 계약을 사용하고 프로젝트 snapshot이나 receipt에 기록하지 않는다.

| 경로 | 계약 |
| --- | --- |
| `GET /workspace` | 프로젝트·세션·run·export·선택 가능한 자료·서버 소유 `document_types` 표시/저장 카탈로그·`document_generation_order` 생성 순서·현재 cursor |
| `POST /sessions` | `title`, `intro`, `context_paths`로 생성하고 대화 run 접수 |
| `GET /sessions/{id}` | 세션과 현재 초안의 readiness 문제·cursor |
| `POST /sessions/{id}/messages` | `content` 저장과 대화 run |
| `POST /sessions/{id}/conversation/retry` | 빈 payload로 최근 실패·취소·중단된 대화 재요청. 종료 확인·idle·revision 조건을 검사하고 `{session, run}` 반환 |
| `GET/POST /sessions/{id}/draft` | 완성 초안 조회 / 문서별 순차 작성·진행 저장·완성 후 전체 비판 검토. 빈 payload는 유효한 미완성 generation 재개, `{restart:true}`는 새 생성 |
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
| `GET /files/tree` | 보기 가능한 재귀 `entries`와 평면 `files`, 잘림 여부·프로젝트 표시 경로. Markdown/지원 텍스트만,2MiB/파일·2000파일·깊이12 등 탐색 예산 적용 |
| `GET /files/read?path=...` | `{path,name,kind,content,size,hash,absolute_path}`. 고정 프로젝트 경계·허용 이름·UTF-8·크기·no-follow 재검사 |
| `GET /transcription/settings` | `{configured,model,timing_supported,max_audio_bytes,timeout_seconds,key_source}`. 원문 키는 반환하지 않음 |
| `PATCH /transcription/key` | 직접 `{api_key?,model?}`. 키 생략은 유지, null/빈 문자열은 삭제. 공용 비밀 파일만 갱신 |
| `POST /transcription` | `audio` 한 필드의 multipart. `{text,model,segments:[{start,end,text}],timing:"segment"|"none"}` 반환. start/end는 초.20MiB/120초, 동시1개, 자동 retry 없음 |
| `POST /clipboard` | `{text}`를 macOS 시스템 클립보드에 쓰고 `{copied:true}` 반환. 브라우저 복사 실패 시 사용하며 읽기·내용 저장 없음. UTF-8 텍스트 최대2MiB, 요청 최대3MiB,5초 제한 |

`202`는 접수이며 생성 완료가 아니다. SSE delta는 임시 데이터이고 마지막 확정 snapshot이 기준이다. feed를 놓치면 cursor replay 또는 snapshot polling으로 복구한다. UI는 불명확한 저장 응답을 같은 export ID로 조회해 확인하고 새로운 저장을 겹치지 않는다.

## 프로젝트 문서 보기와 녹음

파일 탐색은 기존 AI 자료 선택과 별개다. 탐색은 총32MiB·20,000entry 예산에서 잘릴 수 있고 단일 디렉터리의 기존10,000entry 상한 초과는 `file_unavailable`로 거절한다. 탐색 목록을 읽거나 파일을 열어도 해당 파일을 AI 대화 문맥에 추가하지 않는다. Markdown은 기존 정화 renderer로, 코드·HTML 등은 실행 없이 텍스트로 표시한다. 숨김/민감 파일·symlink·hardlink·비 UTF-8·바이너리와 dependency/build 폴더는 제외한다. 원래 파일명의 Unicode 표현을 보존하고 브라우저 링크는 서버 카탈로그 안에서만 NFC 대응한다. 상대 링크의 `..`는 프로젝트 안으로 해석될 때만 허용하며 절대 경로·외부 자원 요청으로 바꾸지 않는다.

파일 트리·중복 없는 문서 탭·각 읽기 위치·채팅 탭과 우클릭 복사 메뉴는 페이지 메모리에서 관리한다. 프로젝트 파일 목록은 단계 안내 바로 아래에 두고 개인정보 안내·경로 footer와 분리한다. 좌우 제어는 header 메뉴 대신 각 패널의 안쪽 경계에 둔다. 접은 뒤에도 복원 버튼을 화면 가장자리에 남기고 패널 overflow/숨김과 분리한다. 공통 색·글자·간격을 시작/대화/문서/설정 화면에 적용하며 overlay와 고정 도구의 포커스 겹침을 실제 브라우저로 확인한다. 패널 크기가 바뀌면 도표도 재맞춤하며 확대·이동에 내부 스크롤바를 사용하지 않는다. 전체 저장 modal은 파일 목록만 기본으로 표시하고 원문은 접힌 details에 둔다. header/body/footer를 나누어 긴 원문이 저장 버튼을 밀어내지 않는다.

녹음은 설정의 키 존재를 확인한 뒤에만 마이크 권한을 요청한다. MediaRecorder의 실제 start 이벤트를 공통0으로 두고 녹음 중 문서 열기·클릭·선택·복사를 파일/제목/경과시간과 함께 메모리에 기록한다. stop 시각·조작 배열을 즉시 동결하고 지연 start/timer/stop/upload callback은 세대와 phase를 검사한다. 종료 후에는 음성만 고정 OpenAI Audio API로 전송하고 실제 발화 구간의 초를 안전한 정수 ms로 바꾸어 문서 조작과 stable sort한다. 같은 시작 시각은 발화가 먼저이고 같은 종류/시각의 원래 순서는 유지한다. open/focus/selection·word/copy를 open/click/select/copy로 표현하고 파일·앵커·선택 text·줄/offset을 보존한다. 통합 Markdown 타임라인을 클립보드에 쓴다. 시간 없는 응답은 untimedSpeech로 분리하고 전사 길이로 시각을 추정하지 않는다. 정렬된 발화와 전체 원문이 NFC/공백 정리 후 일치하지 않으면 전체 원문도 별도 포함한다. 문서 수정이나 AI 작업을 자동 실행하지 않는다. 페이지 종료·프로젝트 reset 때 마이크 track·파일 요청·전사 요청을 중단하고 지연 결과를 무시한다. 오디오·전사·위치 기록의 디스크 보관은 없다.

복사는 사용자 클릭 직후 조건을 유지하도록 `navigator.clipboard.writeText`를 첫 await 전에 호출한다. 거부되거나 API가 없으면 동일한 Host·Origin·Bearer·프로젝트 유효성 검사를 적용하는 `/clipboard`로 대체한다. 서버는 고정 `/usr/bin/pbcopy`에 stdin으로만 텍스트를 전달하며 shell/임의 명령·클립보드 읽기·원문 로그·파일 보관을 사용하지 않는다. reset/shutdown·시간 초과에는 자식 프로세스를 종료한다. 둘 다 실패하면 원문 전체와 허용 목록의 원인 안내를 수동 복사 창에 표시하고 직접 클릭 재시도를 제공한다. clear/dispose 또는 새 복사 후 늦은 브라우저 실패는 로컬 요청을 시작하지 않으며 진행 중 요청도 취소하고 UI 완료를 무시한다.

키와 전사 모델은 공용 config의 형제 파일 `transcription-secrets.toml`에 별도로 보관한다. 권한0600·소유자·no-follow를 확인하고 atomic 교체하며 일반 `Settings`의 Serialize/Debug·프로젝트 snapshot·receipt에 넣지 않는다. 환경변수 `OPENAI_API_KEY`는 파일 키가 없을 때 대체로 사용할 수 있다. 파일 키를 삭제해도 환경변수 키가 있으면 `configured:true,key_source:environment`로 표시한다. 격리 `App::open`은 실제 공용 파일과 환경 키를 읽지 않는다. 기본 전사 모델은 `whisper-1`이며 기존 비밀 설정 파일에 명시된 모델은 유지한다. Whisper는 verbose_json/segment 시간 정보를 요청하고 다른 모델은 json만 요청한다. Settings의 timing_supported는 알려진 whisper-1 프로필에만 true다. 응답 segments가 누락/null/빈 배열이면 timing:none이며, 실제 Whisper 구간이 있으면 timing:segment다. malformed 구간·음수/역전/비유한/안전 ms 상한 초과·NUL·4096개 초과는 원문 노출 없는 고정 오류로 거절한다. 다른 모델의 시간 정보를 임의 구간으로 변환하지 않는다. UI는 녹음 종료 후 명시적으로 POST 요청을 보내며 이때 원격 API를 호출한다. 연결10초·전체120초·응답1MiB·오디오20MiB 제한, redirect/자동 retry 금지와 reset/shutdown 취소를 적용한다. [공식 API](https://developers.openai.com/api/reference/resources/audio/subresources/transcriptions/methods/create)를 기준으로 음성의 실제 MIME/컨테이너를 검사한다.

## 대화·검토·저장

- 작성 중에는 대화/검토 화면 안의 임시 메시지와 중지 버튼을 표시한다. 임시 CLI JSON은 답변으로 표시하지 않는다. 대화가 완료되면 최신 메시지와 입력창을 보여주며 일반 snapshot과 모달 갱신에서는 읽기 위치를 보존한다.
- 작성 중과 timeout 오류에는 해당 run의 `effective_settings.timeout_seconds`를 표시한다. `시간 제한 조정`은 기존 설정의 해당 입력으로 이동하며 실패 작업을 재시작하지 않는다. `초안 다시 생성`은 기존 draft 생성 API로 새 run만 시작하고 대화 입력을 추가하지 않는다. 이전 메시지·완성 초안과 종료 미확인 차단을 유지한다.
- 대화 후보의 배열 수·문자열 UTF-8 byte·NUL·총량 제한은 검증과 prompt가 같은 상수를 사용한다. 오류는 원문 대신 field/index·실제 크기·한도를 전달한다. 기존 최대 한 번의 형식 보정을 유지하며 실패 후보는 원자적으로 거절한다.
- 실패·중단 화면과 재접속 복구 화면의 **답변 다시 받기**는 종료가 확인된 최근 conversation에만 제공한다. 새 메시지를 추가하지 않고 기존 대화·초안·의미 입력 버전과 실패 이력을 보존하며 현재 공통 AI 설정으로 새 run을 만든다. 완료·활성·종료 미확인 run 및 design/critic 실패에는 이 작업을 허용하지 않는다.
- 질문이 남아 있어도 이야기 나누기의 다음 단계 버튼으로 초안을 만들 수 있다. 새 설계는 [단일 문서 응답 계약](../prompts/design-single-document.md)의 strict `{kind,markdown}`를 고정 순서로 10회 요청한다. 각 문서는 [10종 패키지 계약](../prompts/design-package-format.md)의 해당 품질 규칙을 따르고, 마지막에 기존 `markdown`+정확히 9종 `documents` 패키지로 조립해 전체 검증한다. 모델이 파일명이나 경로를 정할 수 없다. SmartMD·종류별 필수 절·전용 DSL·DRV/QA/ALT/ADR/VAL 연결과 실제 문서 anchor를 검사한다. 형식 오류는 실패로 기록하고 모델을 자동 재호출하지 않는다. 기존 저장 문서는 다시 검사하거나 변환하지 않는다.
- `Session.design_generations`는 완료 Draft와 별개의 진행/생성 설정 기록이다. 문서 요청 전 진행 event를 기록하고, 성공 응답의 검증·hash·source run·설정·CLI 정보를 snapshot/event와 같은 transaction에 저장한 뒤 다음 요청을 시작한다. 매 단계에서 active run·입력 버전·문맥/계약 hash·취소/reset을 확인한다. 재개는 마지막 미완성 generation만 후보로 삼으며 언어·입력·규칙·문서 prefix/hash가 맞아야 한다. 같은 run은 최초 설정을 고정하고 새 resume run은 현재 신뢰된 설정을 사용한다. 저장된 executable/profile로 프로세스를 실행하지 않는다.
- 중간 결과는 Draft·승인·export 대상이 아니다. 마지막 주 문서가 전체 패키지 검증까지 통과해야 기존 Draft와 전체 critic 경로에 넘긴다. 실패/중단 뒤 자동 재개는 없고 종료 미확인 게이트를 유지한다. 완료 문서의 내용을 바꾸려면 새 generation을 시작하며, 마지막 전체 연결 검증 실패에도 앞 문서를 보존하고 재시도/명시 restart 선택을 제공한다.
- 비판 검토에는 주 문서와 상세 문서 전체를 전달한다. 문맥 제한을 넘으면 일부를 잘라 검토하지 않고 명시적으로 실패시킨다. 이미 완성한 설계는 보존하고 검토 실패를 구분한다. 자료가 부족하면 가정·미정·조건부 대안을 남기도록 하며, 정량 목표나 사용자의 승인을 만들어 내지 않도록 지시한다. 구조 검사는 설계 내용의 정확성 보장이 아니다.
- 검토 UI의 문서 종류 선택은 서버 카탈로그와 실제 초안을 조합한다. 카탈로그에 있는 파일 링크만 종류 전환과 로컬 anchor 이동으로 처리한다. 제목 ID는 원문 HTML ID를 신뢰하지 않고 정화 후 생성한다. Markdown·SmartMD·Mermaid와 4종 DSL은 내장 자산으로 표시하고, 원문 보기와 도표별 오류를 제공한다. 문서와 생성 SVG는 따로 정화한다.
- 새 주 문서의 `Method-R system design`에는 시스템 구성도 Mermaid flowchart가 필요하다. 실제 절·도표 연결 유무를 검사하고 구성요소 책임·상태/자료 소유자·화살표 계약은 prompt와 독립 검토로 확인한다. Method-R 매크로/모듈 jobflow와 기존4종 상세 DSL을 대체하지 않으며10종 문서 수와 과거 저장 초안은 유지한다.
- 기본 coverage와 결제/민감 데이터/되돌릴 수 없는 변경 overlay를 적용한다. 범위 크기와 실패 영향을 분리하고, 중요한 미정/위험 질문을 코드가 우선한다. v1 pack은 내장 정책이며 외부 실행 플러그인은 없다.
- AI가 추출한 내용은 해석/가정/제안으로 남긴다. 근거 ID와 원문 인용을 확인하며, 사용자 사실로 자동 승격하지 않는다. 최신 답변이 모름·추천·보류이면 이를 잘라 인용해 확정 수정의 근거로 사용할 수 없다.
- 요구·진술·결정 수정은 새 버전과 `supersedes` 연결을 만들고 이전 근거를 보존한다. 과거 값을 익명 신규 항목으로 부활시키지 않는다. 연결된 결정은 변경 영향을 재검토한다.
- 내용 승인과 파일 저장을 분리한다. 근거가 있는 활성 요구, 필수 coverage, 독립 검토, 주요 지적 처리, 현재 설계 버전이 검토본 확정의 조건이다. 주요 지적을 다시 열면 기존 승인도 철회한다.
- 초안은 미생성/현재/이전 버전으로 구분한다. 과거 초안은 해당 버전으로 저장할 수 있지만 현재 설계의 승인으로 승격하지 않는다.
- preview는 전체 패키지의 source hash·설계 버전·현재 세션 revision에 연결한다. 상세 문서만 바뀌어도 기존 preview를 쓸 수 없다. 전체 내용을 한 번에 승인·저장하며 출력 파일마다 근거와 미정, 지적 처리와 내용 상태를 포함한다. 파일별 hash와 manifest v1 계약은 유지한다.
- Store에서 파생한 `ProjectFiles`는 원래 root 디렉터리 handle을 유지한다. 경로를 symlink로 바꿔도 외부로 쓰지 않는다. 새 bundle을 배타적으로 생성하고 파일은 no-clobber publish, manifest는 마지막에 반영한다. 기존 문서를 덮어쓰지 않는다.

## 설계 대비 현재 범위

| 항목 | 실제 구현 |
| --- | --- |
| 로컬 Rust CLI + 웹 대화 | 포함된 HTML/CSS/JS, Node 실행 의존성 없음 |
| Claude/Codex·모델·생각 수준 | 역할별 선택·고정 run 설정, 실제 CLI stdout 프로토콜 해석 |
| CLI 버전/권한 | 검증한 정확한 버전 프로필, macOS 프로젝트 쓰기 차단. 다른 OS 빌드·named profile은 미지원; 미검증 실행 조건은 거부 |
| 설정 계층 | 공통 TOML의 에이전트·모델·생각 수준·시간 제한 + 프로젝트 DB의 문서 설정. 저장소의 임의 실행파일/args/env는 로드하지 않음 |
| 문서 분할 | 주 아키텍처 + 9종 상세 Markdown과 manifest, 과거 단일 문서 호환. 외부 pack/template과 자동 다중 프로젝트 분할은 확장 지점 |
| 구조와 추적성 | Method-R, 드라이버·품질 시나리오·대안·ADR·검증 ID와 문서 링크. 실제 근거·업무 판단·성능 수치는 사람의 검토 대상 |
| 문서 표시 | 정화된 Markdown·SmartMD·Mermaid·Navigation/Jobflow/State/Layout, 종류 선택과 패키지 내부 링크. HTML 프로토타입 생성 없음 |
| 진단 보존 | stderr의 단계·오류 요약. 파일 logger와 원문 대화 덤프는 기본 비활성 |
| 운영 | CLI/브라우저 재시작·파일 대사·수동 종료 확인. 강제 부모 종료/전원 손실의 자동 고아 정리 보장은 없음 |

이 범위는 초기 사용자 요구의 실행·대화·설계 문서·로컬 AI 설정 경로를 구현한다. 모델 답변의 의미적 정답, 모든 분야의 규칙, 모든 CLI/OS 버전 호환을 기계 검사로 보장하지 않는다.

## 검증 근거

[실행 이력 색인](../tests/test-history/README.md)에 최초 실패와 수정·재검증을 보존한다. 소스 코드 단위/대역과 로컬 실물·실제 모델·브라우저 증거를 구분한다. 실제 실행하지 않은 Linux/Windows·전원 손실·장기 부하·실제 disk-full 결과는 통과로 표시하지 않는다.
