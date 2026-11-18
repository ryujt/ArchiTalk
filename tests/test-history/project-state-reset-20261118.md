# 프로젝트 상태 삭제·교체 후 종료 경계

- 범위: Store/App/HTTP/SSE의 프로젝트 상태 경계. 공용 AI 설정과 AI runtime 격리는 각각 root와 별도 담당자 소유다. 실제 사용자 서버·프로젝트 DB 삭제는 root만 수행하며 이 담당자는 격리된 임시 프로젝트만 검증한다.
- 이전 근거: [implementation-storage](implementation-storage.md) ST-08~ST-10의 pinned root/dirfd·프로세스 락 보안은 유지한다. 다만 `.architalk` 이름이 교체되어도 이전 inode에 계속 commit한다는 과거 기대는 새 reset 계약과 충돌하므로 이제 거절한다. [design-package-storage](design-package-storage-20261118.md)의 schema2/backup·발행 복구도 회귀 대상이다.
- 계약: state 디렉터리·DB·락 및 선택 root 이름의 inode 경계가 달라지면 해당 서버는 terminal `workspace_reset_required`다. 이전 snapshot·delta·completion·stop commit을 금지하고 자동 디렉터리 재생성·재연결·메모리 복사를 하지 않는다. API는 HTTP410, 기존 SSE는 같은 이름의 reset event 후 종료한다. 새 서버에서만 새 DB를 연다.
- 사용자 정정: 모델·에이전트 설정은 공용 설정에 유지한다. 대화·실행·캐시·runtime 자료가 프로젝트 `.architalk` 소유다. 공용 설정에 프로젝트 경로나 대화 원문을 쓰지 않는다.

## RST-01 — 삭제·교체·DB unlink·기존 SSE 반례

- 사전 명령: `cargo test --test state_reset -- --nocapture`.
- 기준: 합성 private marker를 담은 실제 SQLite와 App을 연 뒤 state 전체 삭제, 이름 교체, DB leaf만 unlink를 주입한다. 공개 GET/Store read-write/stop 및 기존 SSE가 과거 내용을 반환하거나 재저장해서는 안 된다. 실제 사용자 파일과 모델은 사용하지 않는다.
- 상태: 수정 전 재현 실행 직전. active subprocess completion 반례는 AI runtime 격리 API 연결 후 별도로 실행한다.

- RST-01 결과: 실제 공개 경로 **0 pass / 4 fail**, exit101. state 삭제 후 cached Workspace가 marker를 반환했고, DB unlink 후 HTTP200, 교체 Store가 old snapshot을 읽었으며 기존 SSE가 old_record를 재생했다. 제품 결함을 확인했다. 동시 작성 중인 shared-AI hook/runtime helper의 unused 경고는 연결 전 상태로 구분한다.

## RST-02 — terminal identity guard와 HTTP/SSE 연결

- 수정: Store는 visible root/state/DB/lock identity를 NOFOLLOW로 확인하고 mismatch를 terminal latch한다. App은 checked lock으로 이전 payload를 버리고 cancellation을 전달한다. GET/명령/complete/fail_completion/stop/replay와 live delta 경계에 연결했으며 활성 생성 중에는 100ms watcher로 HTTP가 없어도 감지한다. settings도 Result·HTTP410으로 통일한다. 공용 AI sync hook은 root 소유 구현을 호출한다.
- 예정 명령: 소유 Rust 파일 형식, `cargo test --test state_reset --test storage_contract --test files_contract -- --nocapture`. active 실제 subprocess와 원래 workflow 재개방 락 증상은 후속으로 구분한다.

## RST-LOCK-01 — 병렬 workflow의 일시적 project lock 조사

- root가 다른 focused 묶음에서 `critique_preparation_failure_keeps_completed_design_instead_of_pending`의 seed Store drop 직후 App::open에서 project_locked를 관찰했다. 새 guard는 검사용 FD를 열지만 flock을 추가하지 않는다. 다른 시험의 fork→exec 동안 기존 CLOEXEC root flock의 일시 상속 가능성을 단독 실행으로 구분한다. 락 정책 완화·맹목 retry는 하지 않는다.
- 명령: `cargo test --test workflow critique_preparation_failure_keeps_completed_design_instead_of_pending -- --exact --nocapture`. 상태: 실행 직전.

- RST-02 결과: files_contract14 + state_reset4 + storage_contract12 = **30 pass**, exit0. 삭제·교체·DB unlink·기존 SSE 모두 reset 경계를 통과했고 기존 핀/외부 파일 보호·마이그레이션 회귀도 통과했다.
- RST-LOCK-01 단독 결과: **1 pass**, exit0, 실행 0.55초. 병렬 fork 상속 가설은 단독 통과만으로 확정하지 않는다.

## RST-LOCK-02 — 제어된 fork/pre_exec 상속 반례

- 사전 명령: `cargo test --test storage_contract dropping_store_releases_locks_even_while_an_unrelated_child_is_before_exec -- --exact --nocapture`.
- UnixStream gate에서 실제 `/usr/bin/true` 자식을 exec 직전에 멈춘다. 부모 Store 생존 시 재개방 거절, 부모 Store drop 뒤 자식이 여전히 gate에 있어도 재개방 허용이 계약이다. gate는 판정 전에 항상 release/join한다. 수정 전 실패를 먼저 확인하며, 자식·실제 사용자 프로젝트·모델과 무관한 격리 프로젝트만 사용한다.

- 수정 전 결과: **0 pass / 1 fail**, exit101. 살아 있는 부모의 lock은 정상 거절되었지만 부모 drop 뒤에도 gate 자식이 상속한 open-file description이 lock을 유지했다. gate release/exec/join 후 종료했으며 자식을 남기지 않았다.
- 수정: Store::Drop이 소유 종료에서 workspace.lock→project inode 순서로 명시 unlock한다. Store 공개 API는 statement/transaction을 밖으로 반환하지 않으므로 Drop 시 활성 DB 작업이 없다. 살아 있는 Store의 잠금·재시도 정책은 바꾸지 않았다. 예정 명령은 위 동일 반례와 기존 실제 프로세스 경계 시험이다.

- 수정 후 storage_contract 전체 **13 pass**, exit0. gate 반례와 기존 별도 실제 프로세스 lock 시험이 함께 통과했다.

## RST-03 — 조용한 활성 subprocess·symlink 교체

- 사전 명령: `cargo test --test state_reset --test workflow --test shared_settings --test settings_choices -- --nocapture`.
- 합성 Claude 호환 실제 subprocess가 init 후 30초 대기함을 progress로 확인한다. 그 뒤 `.architalk` 삭제/이름교체 후 API 요청 없이 3초 안에 종료되어야 하며 late delta/completion·stop 저장·디렉터리 부활이 없어야 한다. 별도 symlink 교체는 외부 marker와 파일 수 불변으로 확인한다. 유료 모델 호출은 없다. workflow/shared_settings는 계약 회귀만 포함한다.

- RST-03 1차 결과: state_reset6 + workflow7 + shared_settings4 + settings_choices2 = **19 pass**, exit0. 무통신 대기 subprocess의 삭제/교체 취소는 전체1.06초에 통과했다.
- 후속 replay 레코드 사이 재검사 반례를 추가했다. 첫 통합 명령은 fixture helper의 Workspace move/borrow 컴파일 오류 E0382로 시험 실행 전 exit101이었다. 직렬화 인자를 borrow로 수정한 뒤 동일 명령을 재실행한다. 검증 명령: `cargo test --test state_reset --test storage_contract --test files_contract --test workflow --test shared_settings --test settings_choices -- --nocapture` 및 `cargo clippy --lib --test state_reset --test storage_contract --test files_contract --test workflow --test shared_settings --test settings_choices -- -D warnings`.

- 최종 결과: files_contract14 + settings_choices2 + shared_settings4 + state_reset7 + storage_contract13 + workflow7 = **47 pass / 0 fail**, exit0. 활성 삭제/교체·중간 replay/stop·symlink 외부보호를 포함한다. focused clippy `-D warnings`도 exit0. 소유 Rust 파일은 rustfmt로 정리했다. source freeze를 root/UI 담당에게 통지했으며 유료 모델 호출은 수행하지 않았다.

## RST-04 — 비작성 공용 설정 독립 검토

- `src/config.rs`의 save_ai 및 App의 shared 설정 hunk는 root 작성이므로 이 담당자는 읽기 전용으로 검토했다. save_ai가 typed AiConfig만 current.ai에 대입하고 project documents는 DB에 남기므로 경로/대화/run 원문을 global에 넣는 경로는 찾지 못했다. flock 안에서 현재 global을 재로드하고 AI 전체 fingerprint CAS를 비교하여 cooperating writer의 stale 변경을 막는다. executable/profile은 현재 trusted global과 비교한다. 관련 shared4/workflow7도 통과했다.
- 중요한 blocker는 발견하지 않았다. global AI 저장과 project DB commit은 별도 저장소라, 후자만 실패하면 전자는 이미 반영될 수 있다. 다음 settings GET의 shared sync가 실제 global 값을 반영하며 문서경로/대화가 global로 넘어가지는 않는다. 이 검토는 자가 작성한 reset guard의 독립 검토를 의미하지 않는다.
- UI N01~N04 이력은 별도 [project-state-ui](project-state-ui-20261118.md)로 이동했으며 이후 이 파일은 RST 담당자만 추가한다.
