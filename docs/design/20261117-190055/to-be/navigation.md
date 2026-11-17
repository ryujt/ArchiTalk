# ArchiTalk Navigation

## 범위·확인 기준

이 문서는 대상 프로젝트의 현재 폴더에서 Rust CLI를 실행한 뒤 사용하는 **목표 UX 제안**이다. 현재 저장소의 구현을 설명하지 않는다. 로컬 프로세스가 제공하는 `/`의 **설계 워크스페이스 한 페이지**에서 시작·인터뷰·검토·저장·복구 장면을 전환한다. 별도의 대화 목록 페이지, 문서 편집 페이지, 로그인 페이지는 만들지 않는다. `Workspace(Scene)`는 이 페이지 안의 주요 단계 또는 오버레이이며, 독립 라우트가 아니다.

HTML은 바닐라 HTML 한 파일에 예시 데이터와 장면 전환만 담는다. 파일을 직접 열어 사용할 수 있고, 서버·AI·파일 저장·설정 변경을 호출하지 않는다. fragment는 이 목업의 안정된 검토용 진입점이다. 실제 앱의 상태·복구 가능 여부는 로컬 프로세스가 판단한다. 직접 여는 검토·저장 장면은 그 장면에 필요한 예시 초안이 있는 조건으로 시작하며, 제품의 생성 전 상태와 구분한다. 전체 [페이지·장면 목록과 Layout](screen-layout.md#workspace), [목표 시스템과 공개 계약](system-design-to-be.md)을 함께 읽는다.

아래 API 노드는 제안하는 로컬 HTTP 계약이다. AI 작업은 로컬에 설치한 **Claude Code 또는 Codex CLI**로 실행하며, 기존 CLI 로그인과 설정을 사용한다. ArchiTalk가 별도 API 자격증명을 받거나 관리하는 화면은 없다. 모델·생각 수준의 지원 여부는 설치된 CLI 버전과 호환성 프로필로 확인한다. 제품 브라우저에서 CLI의 로그인 과정을 대신 진행하지 않는다.

<a id="n01"></a>
## N01. 프로젝트를 소개하고 대화 시작하기

```navigation
Workspace(Welcome) --> (validate_intro) : 대화 시작
(validate_intro) --> Workspace(Welcome) : 소개가 비어 있음
(validate_intro) --> (/api/v1/sessions) : 소개 확인
(/api/v1/sessions) --> Workspace(Interview) : 대화 준비 완료
Workspace(Welcome) --> Workspace(Settings) : 사용할 도구 확인
Workspace(Settings) --> Workspace(Welcome) : 적용 또는 취소
```

`GET /api/v1/workspace`와 현재 세션 조회로 진입 상태를 판단한다. CLI가 알려 준 로컬 주소를 열면 `Welcome`에서 시작한다. 대화 소개와 함께, 질문에 사용할 프로젝트 이름·README 소개·구성 파일 종류의 요약을 먼저 보여 준다. 프로젝트 스캔은 읽기 전용이고 대화에 전달할 범위를 미리 확인할 수 있다. 전체 소스·숨김 파일·인증 정보를 자동으로 전달하지 않는다. 빈 소개는 같은 장면의 입력 검증으로 남고, 대화를 시작하면 새 세션을 준비한 후 한 질문씩 묻는다. 실행 폴더를 바꾸는 기능은 이 페이지의 범위가 아니며 다른 프로젝트는 그 폴더에서 CLI를 실행한다.

로컬 주소 접속 자체를 화면 노드나 화살표로 만들지 않는다. 프로세스가 시작되지 못하는 문제는 터미널 진단 범위다. 브라우저 접속 후 연결·기록 준비 실패는 대화와 파일을 바꾸지 않고 오류를 알리며, 복구·다시 시작은 [N07](#n07)과 연결한다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 시작·인터뷰·연결 설정 | [기본·시작](details/screens/workspace.html#welcome) · [인터뷰](details/screens/workspace.html#interview) · [연결 설정](details/screens/workspace.html#settings) | [배치·장면](screen-layout.md#workspace) |

<a id="n02"></a>
## N02. 규모·분야·중요한 조건을 한 질문씩 구체화하기

동일한 `Workspace(Interview)`에서 이루어지는 과업이다. 답변을 보내면 `/api/v1/sessions/{id}/messages` 계약으로 확인된 대화를 기록하고, 필요한 다음 질문을 만든다. 입력 오류, 빠른 답변 선택, 메모 갱신은 같은 인터뷰 안의 결과이므로 내부 토큰·세부 탭 변화마다 이동 화살표를 만들지 않는다.

처음에는 목적과 분야를 듣고 예상 이용 규모, 가장 먼저 제공할 기능, 중요한 제약을 순서대로 좁힌다. 한 번에 질문 하나와 질문 이유를 보여 준다. ‘아직 정하지 않았어요’는 정상 답변이며 막힌 단계로 취급하지 않는다. 확인된 내용과 가정·미정을 보조 패널에 구분한다. 작게 시작하는 프로젝트에 대규모 인프라나 긴 질문 목록을 강제하지 않는다. 답변의 복잡도에 따라 질문 깊이를 늘리되 사용자가 현재 답변으로 초안을 만들 수 있다.

빈 답변은 `Validation`에서 입력란과 오류를 함께 표시하고 초안 정리나 파일 쓰기를 시작하지 않는다. 답변을 입력하거나 빠른 답변을 선택하면 `Interview`로 돌아간다. 대화의 자동 체크포인트와 문서 저장은 별개다. 사용자가 답변했다고 프로젝트 문서를 자동으로 변경하지 않는다. 응답 실패의 명시 재시도는 [N06](#n06), 생성 중지는 [N03](#n03)을 따른다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 인터뷰·입력 오류 | [기본·시작](details/screens/workspace.html#welcome) · [인터뷰](details/screens/workspace.html#interview) · [답변 입력 오류](details/screens/workspace.html#validation) | [인터뷰와 입력](screen-layout.md#workspace-interaction) |

<a id="n03"></a>
## N03. 첫 초안을 정리하고 중지·다시 시작하기

```navigation
Workspace(Interview) --> (/api/v1/sessions/{id}/draft) : 현재 답변으로 초안 만들기
(/api/v1/sessions/{id}/draft) --> Workspace(Generating) : 정리 시작
Workspace(Generating) --> (/api/v1/runs/{id}/events) : 진행 결과 받기
(/api/v1/runs/{id}/events) --> Workspace(Review) : 검토할 초안 완료
(/api/v1/runs/{id}/events) --> Workspace(Error) : 작업 실패
Workspace(Generating) --> (/api/v1/runs/{id}/cancel) : 정리 중지
(/api/v1/runs/{id}/cancel) --> Workspace(Paused) : 중지 확인
Workspace(Paused) --> Workspace(Interview) : 대화로 돌아가기
Workspace(Paused) --> (/api/v1/sessions/{id}/draft) : 다시 정리하기
```

명시적으로 초안 만들기를 선택하면 확인된 요구와 미정을 모아 `Generating`으로 전환한다. 진행 설명은 실제 실행 단계에 맞춰 표시하고, 근거 없이 완료 비율이나 남은 시간을 단정하지 않는다. 완성된 결과만 `Review`의 검토 가능한 초안으로 승격한다. AI는 선택한 로컬 CLI를 비대화형으로 실행하며, 브라우저는 진행 결과를 SSE로 받는다.

중지를 누르면 즉시 중지 완료로 단정하지 않고 서버의 확인 후 `Paused`를 표시한다. 중지와 완료가 경합하면 서버가 확정한 최종 상태를 따른다. 이미 완료됐으면 완성된 초안 검토로 안내하고, 확인되지 않은 결과는 재진입 시 [N07](#n07)에서 확인한다. 대화·기존 초안·저장된 문서는 보존한다. 미완성 응답은 새 초안에 섞지 않는다. 다시 정리하기는 사용자의 명시적 새 실행이며 자동 재실행하지 않는다.

목업에서 버튼으로 정리를 시작하면 2.2초 뒤 예시 초안으로 전환한다. `#generating` 직접 링크와 검토용 장면 선택은 해당 장면에 고정되어 중지 UI를 살펴볼 수 있다. 이 시간은 제품의 성능 보장이 아니다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 인터뷰·정리 중·중지·검토·실패 | [기본·시작](details/screens/workspace.html#welcome) · [인터뷰](details/screens/workspace.html#interview) · [정리 중](details/screens/workspace.html#generating) · [중지](details/screens/workspace.html#paused) · [초안 검토](details/screens/workspace.html#review) · [작업 실패](details/screens/workspace.html#error) | [작업과 결과](screen-layout.md#workspace-results) |

<a id="n04"></a>
## N04. 초안을 살펴보고 대화로 수정하기

```navigation
Workspace(Review) --> (/api/v1/sessions/{id}/messages) : 수정 요청 전송
(/api/v1/sessions/{id}/messages) --> Workspace(Generating) : 새 초안 정리 시작
Workspace(Generating) --> (/api/v1/runs/{id}/events) : 진행 결과 받기
(/api/v1/runs/{id}/events) --> Workspace(Review) : 변경 초안 완료
(/api/v1/runs/{id}/events) --> Workspace(Error) : 변경 초안 실패
Workspace(Review) --> (/api/v1/sessions/{id}/review) : 이 버전을 검토본으로 확정
(/api/v1/sessions/{id}/review) --> Workspace(Approved) : 현재 버전 수락
```

`Review`에서 목적·범위·주요 책임과 흐름·미확정 사항을 읽고, 보조 패널에서 대화 근거와 제안 문서를 살펴본다. ‘운영자가 실패 내역을 확인하는 방법도 추가해 주세요’처럼 대화로 수정을 요청한다. 완성된 새 초안은 다시 검토하고, 실패하면 마지막 완성 초안을 유지한 채 [N06](#n06)으로 복귀한다. 빈 수정 요청은 현재 입력란의 검증만 표시한다.

문서 패널은 미생성·현재 대화 기준·이전 대화 기준을 구분한다. 생성 전 문서 선택은 생성 안내만 제공하고 예시 완성 문서를 현재 결과로 표시하지 않는다. 초안에는 입력 대화의 설계 기록 번호를 표시하며, 이후 답변으로 오래된 초안은 당시 본문을 보존한 채 재정리 필요 상태가 된다. 이 번호는 의미가 바뀐 입력에만 증가하는 `design_revision`이며, 승인·실행·저장 메타 변경으로는 오래된 초안이 되지 않는다. 오래된 초안도 당시 기준을 표시해 저장할 수 있지만 현재 버전의 승인으로 승격할 수 없다. 현재 버전이고 검토 준비 조건을 충족할 때만 `POST /api/v1/sessions/{id}/review`의 수락으로 `Approved`를 표시한다. 저장 성공만으로 내용이 확정되지 않는다.

보조 패널의 메모·문서 전환과 문서 선택은 이 페이지 안의 읽기 동작이다. 새 페이지나 별도 문서 편집기를 만들지 않는다. 생성된 내용을 사용자가 동의한 요구로 자동 확정하지 않으며, 가정과 미정은 계속 구분한다. ‘문서에 반영’을 누르기 전에는 출력 파일을 바꾸지 않는다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 검토·변경 정리·실패 | [기본·시작](details/screens/workspace.html#welcome) · [초안 검토](details/screens/workspace.html#review) · [수정 입력 오류](details/screens/workspace.html#review-validation) · [검토본 확정](details/screens/workspace.html#approved) · [변경 정리](details/screens/workspace.html#generating) · [작업 실패](details/screens/workspace.html#error) | [검토와 문서](screen-layout.md#workspace-review) |

<a id="n05"></a>
## N05. 자연스러운 문서 이름으로 저장하고 충돌·외부 수정을 처리하기

```navigation
Workspace(Review) --> Workspace(SavePreview) : 문서에 반영
Workspace(SavePreview) --> (validate_output_path) : 확인하고 저장
(validate_output_path) --> Workspace(SavePreview) : 위치나 이름 수정 필요
(validate_output_path) --> (/api/v1/sessions/{id}/exports) : 프로젝트 안의 유효한 경로
(/api/v1/sessions/{id}/exports) --> Workspace(Saved) : 저장 완료
(/api/v1/sessions/{id}/exports) --> Workspace(Conflict) : 같은 이름의 문서가 생김
(/api/v1/sessions/{id}/exports) --> Workspace(ExternalChange) : 미리 본 문서가 외부에서 바뀜
(/api/v1/sessions/{id}/exports) --> Workspace(SaveError) : 쓰기 실패
Workspace(SavePreview) --> Workspace(Review) : 검토로 돌아가기
Workspace(Conflict) --> Workspace(SavePreview) : 이름 다시 정하기
Workspace(Conflict) --> Workspace(ExternalChange) : 기존 문서와 비교
Workspace(Conflict) --> (/api/v1/sessions/{id}/exports) : 새 사본으로 저장
Workspace(ExternalChange) --> Workspace(Review) : 비교를 참고해 다시 검토
Workspace(ExternalChange) --> Workspace(SavePreview) : 새 사본 준비
Workspace(ExternalChange) --> Workspace(Review) : 나중에 결정
Workspace(SaveError) --> Workspace(SavePreview) : 다시 저장 준비
Workspace(SaveError) --> Workspace(Review) : 초안 검토로 돌아가기
Workspace(Saved) --> Workspace(Review) : 문서 살펴보기
Workspace(Saved) --> Workspace(Interview) : 대화 이어가기
```

첫 반영에서는 `docs/architecture/회원-알림-서비스/아키텍처-설계.md`처럼 대화의 프로젝트 이름으로 폴더·문서 이름을 제안한다. 사용자가 저장 전에 수정할 수 있다. timestamp나 세션 ID는 기본 출력명으로 사용하지 않는다. 작은 설계는 이 한 문서로 충분하며, 더 깊은 설계는 같은 폴더의 `요구사항.md`, `결정기록/` 등 필요한 문서만 추가한다. 이 목업의 예시는 한 문서 범위다.

`SavePreview`는 `POST /api/v1/sessions/{id}/exports`의 `mode=preview`로 이번에 쓸 파일 목록·추가/변경 내용·최종 경로·내용 상태를 보여 준다. 기본 ‘초안으로 저장’은 내용 상태를 유지한 채 `mode=apply`를 요청한다. 검토 준비된 현재 버전에만 ‘검토본으로 확정하고 저장’을 제공하며, `/review`의 수락 후 저장한다. 내용 확정과 파일 저장 결과는 별도로 표시한다. 이름 오류는 같은 모달에 남기고, 프로젝트 밖 경로와 상위 경로는 서버에서도 거부한다. 저장 완료 응답을 확인한 뒤에만 `Saved`와 실제 저장 경로를 표시한다. 단순 접수·화면 닫기를 저장 완료로 표시하지 않는다.

동일 이름이 생기면 `Conflict`에서 기존 파일을 보존하고 `docs/architecture/회원-알림-서비스-개정-2/아키텍처-설계.md`처럼 새 개정 폴더에 사본을 저장하는 대안을 제시한다. 이미 있는 파일은 교체하거나 덮어쓰지 않는다. 이후 저장도 새로운 bundle을 사용하며 최신 경로는 로컬 기록에서 관리한다. `ExternalChange`의 ‘비교를 참고해 다시 검토’는 바뀐 파일을 근거로 검토하고 새 사본의 미리보기로 돌아간다. 자동 병합·자동 저장하지 않는다. 목업은 비교와 새 사본 경로를 재현하며 실제 no-clobber 쓰기·재기준 처리는 구현하지 않는다.

권한·공간 부족 등 쓰기 실패는 `SaveError`에 경로와 복귀 동작을 제공한다. 기존 파일과 완성된 초안을 보존한다. 저장 결과를 받지 못했다면 성공으로 단정하지 않고 [N07](#n07)의 복구에서 서버 기록과 파일 결과를 확인한 뒤 재시도한다. 같은 요청을 중복 제출해도 중복 파일을 만들지 않고, 개정 폴더의 신규 파일을 배타적으로 생성하는 보장은 로컬 저장 계약이 소유한다. 저장 결과는 `GET /api/v1/sessions/{id}/exports/{export_id}`로 확인한다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 검토·반영 확인·저장·이름 충돌·외부 수정·쓰기 실패·대화 복귀 | [기본·시작](details/screens/workspace.html#welcome) · [초안 검토](details/screens/workspace.html#review) · [반영 전 확인](details/screens/workspace.html#save-preview) · [저장 완료](details/screens/workspace.html#saved) · [이름 충돌](details/screens/workspace.html#conflict) · [외부 수정](details/screens/workspace.html#external-change) · [저장 실패](details/screens/workspace.html#save-error) · [대화 복귀](details/screens/workspace.html#interview) | [저장과 충돌](screen-layout.md#workspace-save) |

<a id="n06"></a>
## N06. CLI 실행·로그인·응답 실패 후 명시적으로 재시도하기

```navigation
Workspace(Error) --> Workspace(Settings) : 설치와 연결 설정 확인
Workspace(Settings) --> Workspace(SettingsError) : 진단 실패
Workspace(SettingsError) --> Workspace(Settings) : 설정 수정 후 진단 성공
Workspace(Settings) --> Workspace(Error) : 설정 적용
Workspace(Error) --> (/api/v1/sessions/{id}/draft) : 다시 시도
(/api/v1/sessions/{id}/draft) --> Workspace(Generating) : 새 실행 접수
Workspace(Error) --> Workspace(Interview) : 대화로 돌아가기
```

CLI 미설치·실행 실패·로그인 필요·지원하지 않는 모델이나 생각 수준·응답 도중 종료 등은 `Error`에서 마지막 확인 지점과 다시 시도할 대상을 설명한다. 설치·버전·옵션 진단은 `Settings`에서 확인한다. 무료 help 진단은 로그인·모델 지원을 확인하지 않으며 로그인 필요는 실제 실행 오류에 근거해 표시한다. 필요한 설치나 로그인은 해당 CLI의 안내와 실행한 터미널에서 진행하며, 브라우저에 비밀번호·API key 입력란을 만들지 않는다. ArchiTalk는 기존 CLI 인증 파일을 읽거나 복사해 별도 관리하지 않는다.

설정 조회는 `GET /api/v1/settings`, 변경은 `PATCH /api/v1/settings`, 무료 설치·help 진단은 `POST /api/v1/settings/diagnostics`다. 설정 적용만으로 실패한 작업을 자동 재실행하지 않는다. 이전 오류 장면으로 돌아온 뒤 사용자가 ‘다시 시도’를 눌러 새 실행을 시작한다. 대화 복귀도 가능하며 기존 초안·출력 문서는 유지한다. 응답 스트림만 끊긴 경우에는 실행 결과를 먼저 재확인하고, 아직 실행 중인 작업에 중복 실행을 겹치지 않는다. 실행이 이미 종료됐을 때만 명시 재시도를 허용한다.

목업의 실패는 ‘답변 연결 중단’ 한 사례와 ‘CLI 진단 실패’ 사례를 보여 준다. 실제 오류 코드별 설명·터미널 도움말은 구현 시 공통 오류 계약으로 연결할 검증 대상이다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 오류·도구 설정·설정 실패·재시도·대화 복귀 | [기본·시작](details/screens/workspace.html#welcome) · [답변 실패](details/screens/workspace.html#error) · [연결 설정](details/screens/workspace.html#settings) · [진단 실패](details/screens/workspace.html#settings-error) · [재시도 중](details/screens/workspace.html#generating) · [대화 복귀](details/screens/workspace.html#interview) | [오류와 재진입](screen-layout.md#workspace-recovery) |

<a id="n07"></a>
## N07. 새로고침·브라우저 재열기·프로세스 재시작 후 이어가기

```navigation
(resolve_local_session) --> Workspace(Welcome) : 이어갈 대화 없음
(resolve_local_session) --> Workspace(Recovery) : 확인할 기록 있음
(resolve_local_session) --> Workspace(Generating) : 같은 프로세스의 작업이 진행 중
Workspace(Recovery) --> Workspace(Interview) : 이어서 대화하기
Workspace(Recovery) --> Workspace(Review) : 초안 먼저 살펴보기
Workspace(Recovery) --> Workspace(Welcome) : 새 대화 시작
```

로컬 주소를 다시 열거나 새로고침하면 로컬 프로세스가 현재 프로젝트의 세션·초안·최근 실행·저장 결과를 확인한다. 직접 접속은 위 판단에서 시작한다. 같은 프로세스가 살아 있고 실행 중이면 기존 실행에 다시 연결하고 중복 실행하지 않는다. 프로세스가 끝났다면 저장된 마지막 체크포인트에서 `Recovery`를 표시한다. 미완성 답변은 완성된 결과로 처리하지 않는다. 재시작 후 이전 자식 프로세스의 종료가 확인되지 않은 경우 새 AI 실행을 막고, 기존 프로세스 트리의 종료 확인 또는 안내된 수동 정리 뒤에만 다시 시작한다.

‘이어서 대화하기’는 확인된 대화로, ‘초안 먼저 살펴보기’는 마지막 완성 초안으로 돌아간다. 새 대화를 시작해도 기존 기록은 보존한다. 프로세스 재시작만으로 이전 AI 작업이나 결과 불명인 파일 쓰기를 자동 재시도하지 않는다. 저장 결과가 불명확하면 먼저 마지막 반영 기록과 실제 출력 파일을 확인하고, 확인 결과에 맞는 완료·충돌·실패 장면으로 안내한다. 목업 `Recovery`는 ‘대화·초안은 있으나 저장 전’인 예시이며 실제 재시작·DB 복구·결과 불명 대사는 실행하지 않는다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 시작·복구·실행 재연결·대화·초안 | [기본·시작](details/screens/workspace.html#welcome) · [이전 대화 이어가기](details/screens/workspace.html#recovery) · [진행 중 재연결](details/screens/workspace.html#generating) · [대화](details/screens/workspace.html#interview) · [초안](details/screens/workspace.html#review) | [오류와 재진입](screen-layout.md#workspace-recovery) |

<a id="n08"></a>
## N08. 사용할 CLI·모델·생각 수준을 설정하고 원래 과업으로 돌아가기

```navigation
Workspace --> Workspace(Settings) : 연결 설정
Workspace(Settings) --> (/api/v1/settings/diagnostics) : 설치와 버전 진단
(/api/v1/settings/diagnostics) --> Workspace(Settings) : 옵션 진단 성공
(/api/v1/settings/diagnostics) --> Workspace(SettingsError) : 설치나 옵션 확인 필요
Workspace(SettingsError) --> Workspace(Settings) : 설정 수정
Workspace(Settings) --> (validate_cli_capability) : 설정 적용
(validate_cli_capability) --> Workspace(Settings) : 지원 조합 확인 필요
(validate_cli_capability) --> Workspace : 다음 실행 설정 저장
Workspace(Settings) --> Workspace : 취소 또는 닫기
```

어느 단계에서든 ‘연결 설정’을 열고 **Claude / Codex**, 사용자 전역 설정에 준비된 실행 파일 경로, 선택 모델, 생각 수준을 확인한다. 실행파일 변경은 신뢰된 사용자 전역 설정에서만 하며 웹은 준비된 경로를 표시한다. Codex profile은 Codex를 선택한 경우에만 제공한다. 기본 설정을 두고 대화·설계 작성·비판 검토 역할별 CLI·모델·생각 수준을 필요할 때 재정의할 수 있다. 비워 둔 역할은 기본 설정을 따른다.

설치 버전에 맞춰 확인한 추천 모델과 생각 수준만 지원 확인값으로 제시하고, 모델명은 직접 입력도 허용한다. 미확인 조합을 자동 호환으로 간주하지 않으며 ‘지원 미확인’과 진단·수정 동작을 표시한다. 같은 `-p`라도 Claude의 print와 Codex profile 의미가 다르므로 이를 공통 옵션으로 노출하지 않는다. 실제 비대화형 실행 방식은 로컬 CLI 어댑터가 소유한다.

설정 변경은 **다음 생성부터 적용**한다. 실행 중인 작업은 시작할 때 선택한 도구·모델·생각 수준으로 끝낸다. 설정창을 닫거나 Escape를 누르면 연 곳으로 돌아간다. 적용해도 실패 작업을 자동 재시도하지 않는다. 목업은 도구·모델·생각 수준 입력과 진단 상태의 표시를 재현하며, 설치 탐색·로그인·호환성 검증·실제 설정 저장은 수행하지 않는다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 설계 워크스페이스 · 시작·대화·검토에서 여는 연결 설정·진단 실패 | [기본·시작](details/screens/workspace.html#welcome) · [대화 복귀](details/screens/workspace.html#interview) · [검토 복귀](details/screens/workspace.html#review) · [연결 설정](details/screens/workspace.html#settings) · [진단 실패](details/screens/workspace.html#settings-error) | [연결·모델 설정](screen-layout.md#workspace-settings) |

## 검증 범위와 남은 확인

문서·HTML의 동일 페이지 및 장면 대응, 상대 링크, DSL 구조, JavaScript 구문은 정적 검토 대상으로 둔다. 브라우저의 화면 모양·반응형·키보드·dialog 복귀는 통합 검증에서 직접 확인한다. 정적 검토만으로 다이어그램 렌더링이나 실제 CLI·SSE·파일 쓰기가 검증됐다고 판단하지 않는다. 실행 기록과 미검증 범위는 프로젝트의 `tests/test-history/`에 남긴다. 구현 시 CLI 버전별 capability profile, 저장 버전 비교, 취소 경합, 프로세스 재시작 복구를 공개 계약 테스트와 연결해야 한다.
