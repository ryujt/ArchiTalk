# ArchiTalk 로컬 서비스 내부 흐름

한 Rust 서비스의 모듈 관점이다. `SessionController`가 상태·순서를 조율하고 각 worker는 자기 결과를 반환한다. 화살표는 조율자가 연결하는 호출/결과이며 형제 worker의 직접 호출이 아니다. 데이터·오류·동시성 계약은 [시스템 설계 §4~§7](../../system-design-to-be.md)이 원본이다.

## 답변 접수와 모델 후보 확정

```jobflow
orchestrator: SessionController
Object: SessionController, WorkspaceStore, GenerationWorker, InterviewPolicy, HttpGateway
SessionController.SubmitAnswer --> WorkspaceStore.AcceptCommand
WorkspaceStore.AcceptCommand.Accepted --> GenerationWorker.Start
GenerationWorker.OnCandidate --> InterviewPolicy.ValidateCandidate
InterviewPolicy.ValidateCandidate.Valid --> WorkspaceStore.CommitCandidate
WorkspaceStore.CommitCandidate.Committed --> HttpGateway.PublishState
InterviewPolicy.ValidateCandidate.NeedsRepair --> GenerationWorker.RepairOnce
GenerationWorker.OnFailed --> WorkspaceStore.FailRun
WorkspaceStore.FailRun --> HttpGateway.PublishFailure
WorkspaceStore.AcceptCommand.Duplicate --> HttpGateway.ReturnReceipt
WorkspaceStore.AcceptCommand.Conflict --> HttpGateway.ReturnConflict
```

- 답변을 받으면 조율자가 저장소에 명령 키·버전·작업 상태 검사를 맡긴다.
- 새 명령이 수락되면 답변·run·설정 snapshot을 저장한 뒤 조율자가 생성 worker를 시작한다.
- 후보가 도착하면 조율자가 정책에 스키마·근거·revision 검증을 맡긴다.
- 후보가 유효하면 조율자가 저장소에 확정 상태와 최종 메시지 저장을 요청한다.
- 상태와 durable event가 함께 commit되면 조율자가 화면 전달을 요청한다.
- 한 번 고칠 수 있는 형식 오류이면 조율자가 제한된 repair 작업을 요청한다.
- 생성 또는 repair가 실패하면 조율자가 run 실패 기록을 요청한다.
- 실패 기록이 저장되면 화면에 오류와 명시적 재시도 경로를 전달한다.
- 같은 명령의 재요청이면 조율자가 기존 receipt를 돌려준다.
- 버전·키·진행 중 작업이 충돌하면 조율자가 충돌과 최신 상태를 돌려준다.

`AcceptCommand`는 긴 작업 중 transaction을 열어 두지 않는다. candidate의 run/attempt/input revision이 현재 active run과 다르면 취소/오래된 결과로 폐기하며, 이 경우 `Valid`가 될 수 없다. `RepairOnce`도 새 attempt로 기록하며 두 번째 실패는 OnFailed다. 생성 화면에 흐르는 delta는 미확정이고, UI 연결이 끊겨도 최종 결과는 DB에서 조회할 수 있다.

## 생성 worker 확대: CLI 실행과 결과 해석

```jobflow
orchestrator: GenerationWorker
Object: GenerationWorker, CliAdapter, ProcessSupervisor, ResultValidator
GenerationWorker.Start --> CliAdapter.BuildRunSpec
CliAdapter.BuildRunSpec.Compatible --> ProcessSupervisor.Spawn
ProcessSupervisor.OnStdoutFrame --> CliAdapter.ParseFrame
CliAdapter.ParseFrame.FinalCandidate --> ResultValidator.CheckTerminal
ResultValidator.CheckTerminal.Valid --> GenerationWorker.OnCandidate
ProcessSupervisor.OnExit --> CliAdapter.CheckExit
CliAdapter.CheckExit.IncompleteOrFailed --> GenerationWorker.OnFailed
CliAdapter.BuildRunSpec.Incompatible --> GenerationWorker.OnFailed
```

- 생성 worker가 고정된 문맥과 설정으로 adapter에 실행 명세 구성을 요청한다.
- 지원 profile이면 worker가 supervisor에 pipe와 독립 process group을 갖춘 실행을 맡긴다.
- stdout frame이 도착하면 worker가 해당 CLI parser에 해석을 맡긴다.
- 최종 후보가 모이면 worker가 종료 이벤트까지 기다리는 join 지점에서 최종 메시지·출력 형식의 일치를 검사한다.
- 검증을 통과하면 worker가 상위 조율자에 후보를 알린다.
- 프로세스가 종료되면 worker가 adapter에 종료 코드와 최종 frame 상태 확인을 맡긴다.
- 실패하거나 최종 결과가 빠졌으면 worker가 상위 조율자에 실패를 알린다.
- 지원하지 않는 옵션·버전·권한 제한이면 실행하지 않고 상위 조율자에 실패를 알린다.

성공은 **프로세스 exit와 terminal 결과를 모두 수집한 뒤 정확히 한 번** 알린다. 최종 후보 frame만 먼저 도착했다고 성공으로 확정하지 않는다. stderr drain·크기/시간 상한·partial 메시지는 supervisor/adapter의 공개 계약으로 처리한다. CLI cwd는 원본 프로젝트가 아닌 전용 run 디렉터리이며, shell을 거치지 않고 argv/stdin으로 전달한다.

## 취소와 완료의 경쟁

```jobflow
orchestrator: SessionController
Object: SessionController, WorkspaceStore, ProcessSupervisor, HttpGateway
SessionController.CancelRun --> WorkspaceStore.RequestCancel
WorkspaceStore.RequestCancel.MarkedCancelling --> ProcessSupervisor.TerminateGroup
ProcessSupervisor.OnTerminated --> WorkspaceStore.MarkCancelled
WorkspaceStore.MarkCancelled --> HttpGateway.PublishCancelled
WorkspaceStore.RequestCancel.AlreadyTerminal --> HttpGateway.ReturnTerminalState
ProcessSupervisor.OnTerminationUnknown --> WorkspaceStore.MarkTerminationUnknown
WorkspaceStore.MarkTerminationUnknown --> HttpGateway.PublishRecoveryNeeded
```

- 중단 요청을 받으면 조율자가 저장소에 현재 작업의 취소 전이를 요청한다.
- 취소가 먼저 기록되면 조율자가 supervisor에 전체 process group 종료를 요청한다.
- 실제 종료가 확인되면 조율자가 저장소에 cancelled 기록을 요청한다.
- 취소 기록이 commit되면 사용자에게 중단 결과를 전달한다.
- 이미 완료/실패된 작업이면 조율자가 기존 결과를 유지해 돌려준다.
- 종료를 확인할 수 없으면 조율자가 미확인 상태를 기록한다.
- 미확인 기록이 저장되면 화면에 재개 전에 실행 상태를 확인하도록 알린다.

취소가 기록된 run의 late candidate는 폐기한다. termination unknown 동안 새 CLI run은 막아 프로젝트당 한 실행 조건을 유지한다. 재시작 때도 interrupted와 별도로 process_termination=unknown을 보존하고 supervisor 확인 또는 수동 정리 확인 전에는 새 spawn을 막는다. stale PID만으로 자동 종료하지 않는다.

## 문서 출력과 재시작 대사

```jobflow
orchestrator: SessionController
Object: SessionController, DocumentRenderer, WorkspaceStore, ProjectFileGateway, HttpGateway
SessionController.ApplyExport --> DocumentRenderer.RenderPlan
DocumentRenderer.RenderPlan --> ProjectFileGateway.CheckExpectedFiles
ProjectFileGateway.CheckExpectedFiles.FreshDestination --> WorkspaceStore.PrepareExport
WorkspaceStore.PrepareExport --> ProjectFileGateway.ApplyPlan
ProjectFileGateway.ApplyPlan.Applied --> WorkspaceStore.CompleteExport
WorkspaceStore.CompleteExport --> HttpGateway.PublishSaved
ProjectFileGateway.ApplyPlan.PartialOrFailed --> WorkspaceStore.RecordExportProgress
WorkspaceStore.RecordExportProgress --> HttpGateway.PublishSaveRecovery
ProjectFileGateway.CheckExpectedFiles.NameOrSourceConflict --> HttpGateway.PublishFileConflict
```

- 저장을 시작하면 조율자가 renderer에 선택한 document revision의 bytes·hash·상대 이름 구성을 맡긴다.
- renderer가 만든 계획으로 조율자가 파일 gateway에 새 대상 경로와 비교할 이전 문서 검사를 맡긴다.
- 미리보기의 근거가 유효하고 새 대상 경로가 비어 있으면 조율자가 DB에 export intent를 저장한다.
- intent가 commit되면 조율자가 파일 gateway에 새 폴더 생성·임시 쓰기·no-clobber publish·manifest 마지막 저장을 맡긴다.
- 모든 목표 hash와 manifest가 일치하면 조율자가 DB 완료 처리를 요청한다.
- 완료 commit 뒤 사용자에게 실제 저장 경로와 revision을 알린다.
- 일부만 반영되거나 실패하면 조율자가 파일별 단계·오류를 저장한다.
- 실패 기록 뒤 화면에 완료되지 않은 파일과 재시도/새 사본 경로를 알린다.
- 이름이 존재하거나 비교 근거가 바뀌었으면 파일을 바꾸지 않고 충돌 비교와 새 이름을 보여 준다.

현재 명령의 expected_revision과 저장 대상 document_revision/hash는 별개다. 이전 초안을 선택해 저장할 수 있지만 그 버전을 표시하고 현재 세션의 내용 승인으로 승격하지 않는다. 초기 경로 검사만으로 이름을 예약하지 않는다. gateway는 디렉터리 exclusive create·소유 handle과 atomic no-clobber publish로 경합을 처리한다. 기존 문서는 교체하지 않으며 다음 저장은 “개정-2” 같은 새 bundle을 사용한다. 외부 프로세스가 새 bundle을 변경하면 identity/hash 불일치로 중단한다. 외부 편집기 자체의 쓰기·삭제를 막지는 않는다.

input/document revision은 design_revision이며 명령 동시성 revision과 다르다. 생성 완료·승인·출력 메타데이터만으로 문서를 stale 처리하지 않는다. bundle 생성 후 identity 저장 전 crash한 폴더는 자동 채택하지 않고 충돌로 남긴다.

재시작 대사는 다음 별도 진입으로 같은 intent를 사용한다.

```jobflow
orchestrator: SessionController
Object: SessionController, WorkspaceStore, ProjectFileGateway, HttpGateway
SessionController.RecoverExports --> WorkspaceStore.ListIncompleteExports
WorkspaceStore.ListIncompleteExports --> ProjectFileGateway.ReconcileHashes
ProjectFileGateway.ReconcileHashes.AllNewHashes --> WorkspaceStore.CompleteExport
ProjectFileGateway.ReconcileHashes.AbsentOrNewHashes --> ProjectFileGateway.ResumePlan
ProjectFileGateway.ResumePlan.Applied --> WorkspaceStore.CompleteExport
ProjectFileGateway.ReconcileHashes.UnexpectedHash --> HttpGateway.PublishFileConflict
```

- 시작 시 조율자가 저장소에서 미완료 export intent를 조회한다.
- 조회한 intent로 조율자가 파일 gateway에 실제 파일·manifest hash 대사를 맡긴다.
- 모두 새 hash이면 조율자가 중단 직전 완료된 출력을 DB에도 완료로 기록한다.
- 파일이 없거나 예상 새 hash만 있으면 조율자가 미생성 파일부터 같은 bytes로 재개하도록 맡긴다.
- 나머지 파일과 manifest 반영이 확인되면 조율자가 완료 상태를 저장한다.
- 예상 밖 hash는 사용자가 편집했을 수 있으므로 자동 교체하지 않고 충돌 안내로 넘긴다.

복구된 최종 상태는 Workspace 조회와 SSE 재동기화로 사용자에게 전달한다. `ResumePlan` 중의 충돌·실패도 앞선 ApplyPlan과 같은 계약으로 기록/안내한다. 복구 자동 재개는 기존에 확인한 exact bytes의 파일 반영에 한하며, AI 실행·새 내용 생성은 포함하지 않는다.

## 검토 범위

선택한 사건은 답변 수락, CLI 결과 확정, 취소 경쟁, 문서 반영, crash 대사다. 기능 전체를 각각 도표로 늘리지 않고 설치 진단·단순 조회는 시스템 계약표를 참조한다. DB와 파일 원자성, CLI의 격리 수준은 화살표만으로 보장되지 않는다. 이 도표의 실제 렌더링·겹침 확인과 제품 통합 실행은 테스트 이력에서 정적 검토와 구분한다.
