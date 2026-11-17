# ArchiTalk 화면 배치

## 전체 페이지 목록

| 화면 ID | 페이지 이름·Navigation 표기 | 실제 경로·진입점·접근 조건 | 관련 시나리오 | Layout 절·HTML | 근거·확인 상태 |
| --- | --- | --- | --- | --- | --- |
| workspace | 설계 워크스페이스 · `Workspace` | 제안 경로 `/`. 대상 프로젝트의 현재 폴더에서 Rust CLI를 실행한 뒤 알려 주는 로컬 주소로 접속. 같은 페이지에서 단계·오버레이 전환 | [N01 시작](navigation.md#n01), [N02 인터뷰](navigation.md#n02), [N03 초안](navigation.md#n03), [N04 수정](navigation.md#n04), [N05 저장](navigation.md#n05), [N06 실패](navigation.md#n06), [N07 복구](navigation.md#n07), [N08 설정](navigation.md#n08) | [워크스페이스 배치](#workspace) · [HTML 기본](details/screens/workspace.html#welcome) | 신규 TO-BE 제안. 현재 제품·API 구현이나 관찰 결과가 아님 |

페이지 1개, 화면 HTML 1개다. 대화 목록·문서 상세·설정·로그인을 독립 페이지로 늘리지 않는다. 아래 17개 장면은 같은 `workspace.html`에서 직접 열고 전환한다. 모델 작업은 로컬에 설치한 Claude Code 또는 Codex CLI에 맡긴다. 로컬 CLI를 사용하는 것이 모델 연산도 로컬에서 수행한다는 의미는 아니며, 모델 처리는 해당 CLI의 설정을 따른다.

<a id="workspace"></a>
## 설계 워크스페이스 — workspace

### 장면 목록

| 장면 ID·이름 | 관련 시나리오·진입 동작/조건 | 보이는 내용·다음 동작/결과 | HTML의 장면 링크·전환 방법 | 근거·확인/재현 상태 |
| --- | --- | --- | --- | --- |
| welcome · 시작 | N01 새 대화, N07 새 대화 선택 | 프로젝트 소개, 한 질문씩 진행한다는 안내, 전달할 프로젝트 요약. 대화 시작 또는 도구 설정 | [시작](details/screens/workspace.html#welcome), 기본 진입·로고·검토용 장면 선택 | 제안·예시 데이터로 재현 |
| interview · 인터뷰 | N01 시작 성공, N02 답변·빠른 답변, N03 중지 후 복귀, N07 대화 이어가기 | 대화, 질문 하나와 이유, 답변 입력, 확인·미정 메모, 현재 답변으로 초안 만들기 | [인터뷰](details/screens/workspace.html#interview), 대화 시작·빠른 답변·복귀 버튼 | 제안·예시 데이터로 재현 |
| validation · 답변 입력 오류 | N02 빈 답변 보내기 | 대화·답변 입력 유지, 입력 오류 안내와 입력란 연결. 입력 후 정상 인터뷰 복귀 | [답변 오류](details/screens/workspace.html#validation), 빈 답변 전송 | 제안·예시 데이터로 재현 |
| generating · 초안 정리 중 | N03 초안 만들기·다시 정리, N04 수정 요청, N06 재시도, N07 같은 작업 재연결 | 완료된 단계·현재 단계와 정리 중지. 완성 후 검토 또는 중지·오류 | [정리 중](details/screens/workspace.html#generating), 초안 만들기·수정 요청. 직접 링크는 고정 | 제안·정적 예시 진행 상태 재현. 버튼 진입만 2.2초 뒤 예시 완료 |
| paused · 초안 정리 중지 | N03 사용자의 중지 후 서버 확인 | 확인된 대화·기존 초안 보존 안내, 대화 복귀 또는 명시 새 실행 | [중지](details/screens/workspace.html#paused), 정리 중지 | 제안·확인 완료 예시 재현. 실제 취소 경합은 미구현 |
| review · 초안 검토 | N03 초안 완료, N04 수정 결과, N05 검토 복귀, N07 초안 조회 | 주요 설계 본문, 미정·가정, 수정 요청, 문서 반영 진입. 메모·문서 패널 | [초안 검토](details/screens/workspace.html#review), 생성 예시 완료·문서 살펴보기 | 제안·예시 초안 재현 |
| approved · 검토본 확정 | N04 현재 버전 수락, N05 확정 후 저장 | 현재 내용 버전의 수락 상태. 파일 저장과 별개이며, 다음 수정은 다시 검토 필요 | [검토본 확정](details/screens/workspace.html#approved), 검토본으로 확정 | 제안·내용 수락 예시 재현. 서버 revision 검증은 미구현 |
| review-validation · 수정 요청 입력 오류 | N04 빈 수정 요청 보내기 | 기존 초안과 검토 문맥을 유지하고 요청 입력란에 오류. 수정 전송 후 새 초안 정리 | [수정 요청 오류](details/screens/workspace.html#review-validation), 검토의 빈 요청 전송 | 제안·예시 데이터로 재현 |
| save-preview · 문서 반영 전 확인 | N05 문서에 반영, 충돌 이름 변경, 저장 실패 후 재준비 | 상대 저장 경로 편집, 이번 파일·변경 요약, 최종 저장. 현재 내용 상태와 초안 저장·검토본 확정 후 저장을 구분 | [반영 전 확인](details/screens/workspace.html#save-preview), 문서에 반영·이름 다시 정하기 | 제안·입력 검증·새 파일 확인 재현. 실제 diff 엔진은 미구현 |
| saved · 문서 저장 완료 | N05 확정된 저장 결과 | 저장 문서 수·상대 경로, 문서 보기·대화 이어가기 | [저장 완료](details/screens/workspace.html#saved), 확인하고 저장·새 사본 저장 | 제안·완료 장면 재현. 실제 파일은 쓰지 않음 |
| conflict · 이름 충돌 | N05 미리보기 후 동일 이름 파일 생성 | 기존 파일과 새 사본 이름, 비교·새 사본 저장·이름 변경. 즉시 덮어쓰기 없음 | [이름 충돌](details/screens/workspace.html#conflict), 검토용 장면 선택 | 제안·충돌 오버레이 재현. 실제 파일 경합은 미구현 |
| external-change · 외부 수정 | N05 미리 본 파일 내용 변경, 충돌 파일 비교 | 기존 최신 문서와 새 초안 차이, 비교를 참고해 재검토·새 사본·취소. 재검토 후 자동 저장 없음 | [외부 수정](details/screens/workspace.html#external-change), 충돌의 기존 문서와 비교·검토용 선택 | 제안·비교 오버레이와 재검토 연결 재현. 실제 병합은 미구현 |
| error · 답변 연결 실패 | N03/N04 AI 실행 실패, N06 실행 결과 확인 후 재시도 가능 | 마지막 대화·초안 보존, 오류 이유, 도구 설정·명시 재시도·대화 복귀 | [답변 실패](details/screens/workspace.html#error), 검토용 장면 선택 | 제안·응답 중단 사례 재현. CLI 오류별 원인 매핑은 구현 검증 대상 |
| recovery · 이전 대화 이어가기 | N07 저장된 기록이 있는 프로젝트로 재진입 | 대화 주제·마지막 기록·초안 상태, 대화·초안 복귀·새 대화. 자동 AI 재실행 없음 | [이어가기](details/screens/workspace.html#recovery), 검토용 장면 선택 | 제안·저장 전 초안이 있는 예시. 실제 재시작·기록 대사는 미구현 |
| settings · 연결 설정 | N01/N06/N08 헤더 또는 오류에서 열기 | Claude/Codex·전역 설정의 실행파일·Codex profile, 설치·버전 진단과 로그인 미확인 구분, 기본/역할별 CLI·모델·생각 수준. 다음 생성 적용 | [연결 설정](details/screens/workspace.html#settings), 헤더 연결 설정 | 제안·입력과 예시 진단 재현. 실제 도구를 실행하거나 설정 저장하지 않음 |
| settings-error · 도구 진단 실패 | N06/N08 도구 미설치·버전/로그인 확인 실패 | 설치 위치·로그인 필요 안내, 설정 수정과 재진단. 원래 작업으로 복귀 후 명시 재시도 | [진단 실패](details/screens/workspace.html#settings-error), 검토용 장면 선택 | 제안·실패 오버레이 재현. 실제 로그인은 해당 CLI의 책임 |
| save-error · 문서 저장 실패 | N05 쓰기 권한·공간 부족 등 쓰기 실패 | 실패 경로와 기존 문서 보존, 다시 저장 준비·초안 복귀 | [저장 실패](details/screens/workspace.html#save-error), 검토용 장면 선택 | 제안·기존 파일 보존 사례 재현. 실제 파일 오류·원자적 쓰기는 미구현 |

`Workspace(Welcome)` 등 Navigation 표기는 위 장면 이름과 대응한다. `SavePreview`, `ExternalChange`, `SettingsError`, `SaveError`는 각각 `save-preview`, `external-change`, `settings-error`, `save-error`다. 직접 링크에 없는 미정의 fragment는 시작 장면으로 표시한다. 저장 전 이름 검증·Codex 전용 필드·보조 패널 선택은 해당 장면 안의 조작으로 재현한다.

### 넓은 화면의 기본 배치

```layout
Screen V PreviewControls, ApplicationHeader, ApplicationBody
ApplicationHeader > BrandAndProject : 65, WorkspaceActions : 35
BrandAndProject > Brand, CurrentProject
WorkspaceActions > LocalConnectionStatus, ConnectionSettings
ApplicationBody > ProgressSidebar : 18, ConversationWorkspace : 56, SupportingPanel : 26
ProgressSidebar V WorkspaceIdentity, CurrentConversation, InterviewProgress, CheckpointNotice
ConversationWorkspace V PhaseHeader, CurrentStage, WorkspaceFooter
PhaseHeader > PhaseTitle, PhaseStatus
CurrentStage V ConversationOrResult, AnswerComposer, ComposerHelp
SupportingPanel V SupportingTabs, ActiveSupportingContent
ActiveSupportingContent V ConfirmedFacts, OpenQuestions, ProjectContextScope
```

가로 배분은 중심 대화가 가장 넓고 메모가 그 옆에 온다는 도해용 상대 가중치다. 목업은 실제 CSS grid로 기본 왼쪽 218px·오른쪽 316px·중앙 가변을 사용한다. 1,200px 이하에서는 양옆 폭을 줄이며, 1,030px 이하에서는 왼쪽 진행 영역을 숨겨 대화와 근거 패널 공간을 확보한다. 화면·배치 구조가 서비스·모듈 경계를 정하는 것은 아니다.

| 영역 | 역할·입출력·상호작용 |
| --- | --- |
| PreviewControls | 제품 밖의 검토 전용 표시. 모형임을 명시하고 17개 장면을 선택하며 Navigation 문서로 연결한다. 파일·설정·외부 연결을 실제 변경하지 않는다는 사실을 항상 보인다 |
| ApplicationHeader | 제품명·현재 프로젝트 이름·로컬 워크스페이스 상태·연결 설정. 현재 폴더의 정체성을 보여 주고 다른 프로젝트로 임의 전환하지 않는다 |
| ProgressSidebar | 현재 대화와 프로젝트 이해→요구·조건→검토·문서화의 큰 진행만 표시. 세부 체크리스트나 처음부터 모든 질문을 늘어놓지 않는다 |
| PhaseHeader | 지금 해야 할 과업과 진행 상태. 인터뷰·정리·검토·복구에 맞는 문구로 변경 |
| ConversationOrResult | 평소에는 사용자·ArchiTalk 대화, 초안 정리·검토·오류·복구에서는 아래에 설명하는 단계별 콘텐츠 |
| AnswerComposer | 텍스트 답변과 전송. Enter로 전송, Shift+Enter로 줄바꿈하며 한글 조합 중 Enter는 전송하지 않는다. 빈 입력은 장면에 맞는 오류와 연결 |
| ComposerHelp | 대화 기록과 문서 저장의 차이를 짧게 알리고 ‘현재 답변으로 초안 만들기’를 제공. 답변했다고 출력 파일을 바꾸지 않는다 |
| SupportingTabs | 설계 메모·문서 탭. 키보드 좌우·Home·End로 선택하고 읽을 영역을 바꾼다. 독립 페이지로 이동하지 않는다 |
| ConfirmedFacts·OpenQuestions | 확인한 요구는 대화 근거와 함께, 미정은 별도 표현. 사용자 직접 진술과 모델 제안을 섞지 않고 ‘모름’을 임의 수치로 채우지 않는다. 생성된 가정을 사용자가 확정한 사실처럼 표시하지 않는다 |
| ProjectContextScope | 처음 보낼 프로젝트 정보의 요약과 참고 범위. 읽기 전용 제한 스캔·전달할 내용 미리보기·전체 소스와 인증 정보 제외를 알린다 |
| WorkspaceFooter | 초안의 가정·결정을 저장 전에 검토하도록 안내. 내부 API·SSE·DB 이름은 제품 본문에 노출하지 않는다 |

<a id="workspace-interaction"></a>
### 시작과 인터뷰

```layout
InterviewStage V ConversationTranscript, CheckpointStatus, ReplyArea
ConversationTranscript V IntroductionTurn, UserProjectTurn, CurrentQuestionTurn
CurrentQuestionTurn V OneQuestion, QuestionReason, QuickAnswers
QuickAnswers > EstimatedScale, UnknownYet, NarrowScope
ReplyArea V InputError, AnswerInput, ReplyActions, DraftShortcut
ReplyActions > KeyboardHint, SendAnswer
```

시작은 큰 제목·간단한 설명·서비스 소개 입력과 ‘대화 시작’을 중심에 둔다. 기술 선정·상세 규모를 먼저 요구하지 않는다. 보조 패널에서 대화에 사용할 프로젝트 요약을 읽을 수 있다. 인터뷰는 질문 하나를 가장 최근 위치에 두고 ‘왜 묻나요?’와 빠른 답변을 붙인다. 모름도 유효한 답변이다. 목업 답변은 메모리에만 남으며 실제 체크포인트를 쓰지 않는다.

빈 답변은 `validation`, 빈 수정 요청은 `review-validation`에서 오류를 입력란과 `aria-describedby`로 연결하고 초안 검토 문맥을 유지한다. 오류 후 입력에 초점을 둔다. 질문·메모·문서 탭 선택은 현재 페이지의 내용 교체이며 브라우저 페이지 전환을 만들지 않는다. 실제 앱은 추가 질문이 필요 없는 작은 설계에 불필요한 인터뷰 단계를 강제하지 않는 것을 검증해야 한다.

<a id="workspace-results"></a>
### 초안 정리·중지·완료

```layout
GenerationStage V AssistantExplanation, GenerationStatusCard
GenerationStatusCard V StatusSymbol, GenerationTitle, GenerationDescription, ActualStageList, CancelGeneration, PreservationNotice
ActualStageList V ConfirmedRequirements, ResponsibilitiesAndFlow, FailureCasesAndOpenDecisions
```

생성 중에는 현재 실행 단계와 중지 동작을 보여 준다. 근거 없는 ‘87%’나 남은 초를 표시하지 않는다. 서버가 중지를 확인하기 전까지는 중지 요청 중과 완료를 구분해야 하며, 목업은 중지가 확인된 `paused` 결과를 재현한다. 완성 전 결과는 검토 가능한 문서로 보이지 않는다. 중지·실패 시 마지막 완성 초안과 저장 문서를 보존한다. 버튼으로 시작한 목업의 2.2초 완료는 검토 편의를 위한 예시다. fragment 직접 진입은 고정되어 로딩 장면을 확인할 수 있다.

<a id="workspace-review"></a>
### 초안 검토와 수정

```layout
ReviewStage V AssistantSummary, DocumentPreview, ReviewActions, RevisionComposer
DocumentPreview V DocumentStatus, DraftArticle
DraftArticle V ServicePurpose, InitialScope, ResponsibilityFlow, UnconfirmedConditions
ReviewActions > SaveStatusExplanation, PrepareExport
RevisionComposer V RevisionValidation, RevisionInput, RevisionSubmit
```

본문은 읽을 수 있는 실제 문서 형태로 목적·범위·간단한 흐름·미정을 담는다. 빈 영역 이름 상자만 두지 않는다. 수정은 아래 대화 입력으로 요청한다. ‘문서에 반영’은 저장 전 확인 오버레이를 연다. 기본은 초안 저장이며, 검토 준비된 현재 버전만 ‘검토본으로 확정하고 저장’을 선택할 수 있다. `approved` 내용 상태와 디스크 저장 상태를 따로 보여 준다. 아직 반영하지 않은 초안을 저장 완료로 표시하지 않는다.

문서 보조 탭은 미생성·현재 대화 기준·이전 대화 기준을 구분하며, 생성된 문서 이름과 제안 경로를 보여 준다. 생성 전에는 생성 동작을 안내한다. 초안의 대화 기록 번호는 서버 `input_revision`의 `design_revision`과 대응하며 새 답변 후 오래된 초안은 당시 본문을 보존하며 현재 승인으로 승격할 수 없다. 오래된 초안 자체의 저장은 허용한다. 승인·실행·저장 메타만 바뀐 경우에는 설계 기록 번호를 올리지 않는다. 작은 예시는 문서 한 개로 제한한다. 생성된 문서를 확인하는 별도 페이지나 범용 Markdown 편집기를 추가하지 않는다. 외부 변경을 재검토한 목업은 기존의 ‘보관 기간 30일’을 참고 근거로 표시하고, 사용자 확인 전 확정하지 않는다. 실제 텍스트 diff·문서 전체 편집·병합은 이 모형의 범위가 아니다.

<a id="workspace-save"></a>
### 저장 확인·이름 충돌·외부 수정

```layout
SaveDialog V SaveDialogHeader, SaveDialogBody, SaveDialogActions
SaveDialogHeader > SaveTitle, CloseSaveDialog
SaveDialogBody V EditableRelativePath, ChangedFileSummary, ChangePreview, PreservationExplanation, ContentAndExportStatus
SaveDialogActions > ReturnToReview, ConfirmSave
```

```layout
ConflictDialog V ConflictHeader, ConflictExplanation, ExistingPath, SuggestedCopyPath, PreservationWarning, ConflictActions
ConflictHeader > ConflictTitle, CloseConflict
ConflictActions > RenameOutput, CompareExisting, SaveNewCopy
```

```layout
ExternalChangeDialog V ExternalChangeHeader, ExternalChangeExplanation, ComparedDocumentPath, ComparisonColumns, RecheckWarning, ExternalChangeActions
ExternalChangeHeader > ExternalChangeTitle, CloseComparison
ComparisonColumns > LatestProjectDocument : 50, ProposedDraft : 50
ExternalChangeActions > DecideLater, PrepareNewCopy, ReviewLatestContent
```

모달은 배경 위에 표시하고 배경 조작을 막는다. 저장 경로는 현재 프로젝트 기준 상대 경로다. 기본 제안은 `docs/architecture/회원-알림-서비스/아키텍처-설계.md`이며 이름을 수정할 수 있다. 저장할 파일과 추가·변경 내용을 최종 동작 바로 위에 둔다. 절대 경로·상위 경로·빈 이름은 오류를 표시하고 모달에 남긴다. 경로 보안의 최종 판정은 서버가 소유한다.

충돌에서는 기존 파일과 `회원-알림-서비스-개정-2/아키텍처-설계.md` 새 사본 경로를 함께 보여 준다. 저장마다 새 bundle을 배타적으로 만들며 기존 파일 교체·덮어쓰기는 제공하지 않는다. 외부 변경 비교는 지금 디스크의 내용과 초안을 나란히 보여 준다. 비교를 참고해 다시 검토한 경우 검토→반영 확인으로 돌아가 기존 파일을 근거로 살펴보고 새 사본을 저장한다. 원래 문서는 유지한다. 모형에서 실제 diff·해시 비교·원자적 쓰기는 수행하지 않는다.

저장 완료는 문서 수·최종 상대 경로·다음 동작을 담은 카드로 바뀐다. 쓰기 실패는 같은 경로·문서 보존·초안 복귀를 제공한다. 완료 응답이 오기 전에는 완료 표시로 전환하지 않는 것이 제품 구현의 검증 기준이다.

<a id="workspace-recovery"></a>
### 오류와 재진입

```layout
RecoveryStage V RecoverySymbol, RecoveryTitle, PreservedStateSummary, RecoveryActions, NoAutomaticRetryNotice
PreservedStateSummary V ConversationTopic, LastConfirmedTime, DraftAndExportState
RecoveryActions > ContinueConversation, InspectDraft, StartAnotherConversation
```

오류는 의미 있는 이유와 보존된 내용, 하나의 분명한 복구 동작을 먼저 보여 준다. CLI 실행·설치·로그인·지원 조합 문제는 도구 설정으로 연결하고, 설정 후에는 원래 오류로 돌아와 사용자가 재시도한다. 제품에서 스택 추적·원문 CLI 출력·비밀 값·서버 내부 경로를 기본 노출하지 않는다. 목업은 연결 중단과 쓰기 실패의 대표 사례를 재현한다.

이전 기록이 있는 프로젝트를 열면 주제·마지막 기록·문서 상태를 확인하고 대화 또는 초안으로 돌아간다. 로컬 프로세스가 살아 있는 실행은 동일 실행으로 재연결한다. 프로세스 재시작으로 중단된 작업은 자동으로 AI 요청을 다시 보내지 않는다. 이전 자식 프로세스 종료가 미확인인 경우 종료 확인 또는 안내된 수동 정리 전에는 새 실행을 막는다. 기록이 없는 빈값은 시작 장면으로 표시한다. 저장 결과 불명·로컬 기록 손상·CLI 전체 종료는 시스템 계약에서 판단하고, 실제 복구 시나리오 검증이 남아 있다. 목업만으로 재시작 복구를 검증했다고 보지 않는다.

<a id="workspace-settings"></a>
### CLI·모델·생각 수준 설정

```layout
SettingsDialog V SettingsHeader, SettingsBody
SettingsHeader > SettingsTitle, CloseSettings
SettingsBody V LocalCLIExplanation, DiagnosticFailureNotice, CLIFields, DiagnosticStatus, DiagnoseCLI, DefaultAISettings, RoleOverrides, ApplyTimingNotice, SettingsActions
CLIFields V CLIAndExecutable, CodexProfile
CLIAndExecutable > CLIChoice : 40, ExecutablePath : 60
DiagnosticStatus > InstallationStatus, VersionStatus, LoginStatus
DefaultAISettings V DefaultSettingsTitle, DefaultModelAndEffort, CapabilityStatus
DefaultModelAndEffort > DefaultModel : 55, DefaultThinkingLevel : 45
RoleOverrides V ConversationOverride, DraftOverride, CritiqueOverride
ConversationOverride > ConversationModel : 55, ConversationThinkingLevel : 45
DraftOverride > DraftModel : 55, DraftThinkingLevel : 45
CritiqueOverride > CritiqueModel : 55, CritiqueThinkingLevel : 45
SettingsActions > CancelSettings, ApplyToNextGeneration
```

| 영역 | 역할·입출력·상호작용 |
| --- | --- |
| CLIChoice·ExecutablePath | 로컬 Claude Code / Codex 선택과 사용자 전역 설정에 준비된 실행 파일 이름·경로 표시. 새 실행파일 변경은 터미널의 신뢰된 사용자 설정에서만 수행 |
| CodexProfile | Codex에서만 보이는 선택 profile. Claude의 print 옵션과 같은 입력으로 취급하지 않는다 |
| DiagnosticStatus·DiagnoseCLI | 설치·버전은 무료 help 진단, 로그인·모델 응답은 미확인으로 구분. 실제 실행이 로그인 필요를 반환한 경우 해당 CLI 터미널 로그인 안내 |
| DefaultModel·DefaultThinkingLevel | CLI 기본값 사용 또는 모델 직접 입력. 설치 버전에서 확인된 추천 후보와 지원 생각 수준만 선택 가능. 미확인 조합은 ‘지원 미확인’ |
| RoleOverrides | 대화·설계 작성·비판 검토 역할별 CLI·모델·생각 수준. 비우거나 기본 설정을 선택하면 위 기본값을 따름. 별도의 CLI 자격증명은 받지 않음 |
| CapabilityStatus | 실제 CLI 버전별 호환성 프로필과 확인 여부. 모델명의 존재와 선택 모델의 생각 수준 지원을 서로 다른 판단으로 처리 |
| ApplyTimingNotice | 다음 생성부터 적용됨을 명시. 진행 중인 실행은 변경 전 스냅샷 사용. 설정 적용이 실패 작업 재실행을 뜻하지 않음 |
| SettingsActions | 적용 또는 취소 후 연 곳으로 복귀. Escape와 닫기도 동일한 복귀. 시작·오류·생성·검토 중 어디서 열었는지 보존 |

목업은 처음에 ‘진단 전·지원 미확인’을 보여 주고, ‘설치·버전 진단’은 실제 실행 대신 검토용 진단 예시를 표시한다. 로그인은 여전히 ‘요청 전 미확인’으로 남긴다. 그 뒤 선택 가능한 low/medium/high 및 모델 후보도 검토용 옵션 프로필의 예시일 뿐, 실제 설치 환경·모델의 지원 검증 결과가 아니다. 무료 help 성공으로 로그인·모델·생각 수준 지원을 확정하지 않는다. 제품 구현에서는 설치 버전에 대한 검증된 호환성 프로필 없이는 지원을 확정하면 안 된다. 인증 정보는 CLI가 기존 방식으로 관리한다. ArchiTalk가 API key·endpoint를 받거나 자격증명 저장소를 새로 만드는 UI는 없다.

### 좁은 화면과 접근성

```layout
Screen V PreviewControls, CompactHeader, ConversationWorkspace, CollapsibleSupportingPanel
CompactHeader > Brand : 45, ShowDesignNotes : 25, OpenSettings : 30
ConversationWorkspace V PhaseHeader, CurrentStage, WorkspaceFooter
CurrentStage V ConversationOrResult, AnswerComposer, ComposerHelp
CollapsibleSupportingPanel V SupportingTabs, SelectedSupportingContent
```

760px 이하에서는 대화 한 열을 우선한다. 왼쪽 진행 영역은 숨기고, 보조 패널은 헤더의 ‘설계 메모’로 펼쳐 아래에서 읽는다. 내용이 사라지거나 별도 모바일 페이지가 되지 않는다. 문서의 긴 상대 경로는 줄바꿈한다. 저장·오류의 주요 버튼은 세로로 배치하고, 비교 모달의 두 열도 위아래로 바꾼다. 설정 모달은 뷰포트 안에서 스크롤하며 긴 설정을 볼 수 있다.

기본 읽기 순서는 헤더→현재 과업→대화/초안→입력→보조 정보다. 색만으로 완료·오류를 구별하지 않고 아이콘과 문구를 함께 쓴다. 주요 버튼과 입력에 보이는 초점 스타일이 있고 탭에 이름·선택 상태를 연결한다. 모달은 네이티브 `dialog`로 초점을 가두며 배경을 비활성화한다. 닫기·Escape는 원래 단계로 돌아간다. 장면 변경은 별도 상태 안내로 알린다. 실제 키보드 복귀·스크린리더 읽기 순서·좁은 폭의 가로 넘침은 브라우저 검증 대상이다. 이번 환경은 브라우저 접근 host가 없어 통합 브라우저 확인을 실행하지 못했으며, 화면 모양 검증 완료로 보지 않는다.

## 검증과 한계

대상은 페이지 1개·장면 17개·시나리오 8개다. 작성 대상에서 제외한 독립 페이지와 미재현 필수 장면은 없다. 단, 현재 저장소에 제품이 없으므로 이 목록은 구현 조사 결과가 아닌 사용자 요구를 바탕으로 한 설계 제안이다. 실제 동작이 필요한 CLI·SSE·취소 경합·복구·파일 쓰기·버전 비교·호환성 확인은 모형의 범위에 포함되지 않는다.

정적 확인은 목록↔HTML 장면 식별자·문서 링크·Navigation 시나리오별 화면 확인 링크·Layout 트리·JavaScript 구문 범위다. 다이어그램 렌더러 출력과 브라우저 모양·폭·키보드 동작은 별도로 기록해야 한다. 실제 수행 결과·남은 확인·실행 이력은 프로젝트의 `tests/test-history/`에 보존한다. [Navigation](navigation.md)의 모든 시나리오에서 해당 장면을 직접 열 수 있다.
