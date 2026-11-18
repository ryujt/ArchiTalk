# 실패한 대화의 명시적 재시도

- 소유: App retry_conversation 분기, HTTP API route/handler, tests/conversation_retry.rs. Domain 한도/프롬프트 및 UI 버튼은 별도 담당자 소유다. 실제 사용자 DB/서버 재시도·변경과 모델 호출을 하지 않는다.
- 계약: POST `/api/v1/sessions/{id}/conversation/retry`에 기존 command envelope를 사용하며 HTTP202 `{session,run}`을 반환한다. 최신 해당 세션 run이 conversation이며 failed/cancelled/interrupted이고 termination이 confirmed/manually_confirmed인 경우만 허용한다. revision·receipt idempotency·전역 check_idle·수동 종료 확인을 재사용한다. 원래 사용자 메시지/설계 의미/design_revision/초안은 보존하고 현재 shared AI 설정으로 새 run만 만든다.

## RETRY-01 — 공개 API 반례 선행

- 사전 명령: `cargo test --test conversation_retry -- --nocapture`.
- 합성 SQLite 세션만 사용하고 executable은 존재하지 않는 임시 경로라 모델을 호출할 수 없다. 의미/초안 보존·shared AI 최신값·대상 최신 run/상태/종료 조건·명시 recover·HTTP202/revision/idempotency를 검증한다. 수정 전 새 operation과 route가 없어 실패해야 한다.

- 수정 전 결과: **0 pass / 4 fail**, exit101. 기존 operation은 not_found, HTTP route는404로 재현됐다. 구현은 기존 command receipt/revision 검사 뒤 retry 분기에서 전역 idle 및 최신 run 자격을 확인하고 prepare_run만 호출하며 add_answer를 호출하지 않는다. manually_confirmed는 기존 recover의 정상 종료 확인값으로 허용한다.
- 구현 후 1차: **2 pass / 2 fail**, exit101. 두 실패는 GET Session에 부가된 cursor/readiness_issues를 persisted DTO와 직접 비교한 테스트 oracle 오류였다. semantic 비교에서 이 computed/transport 필드만 제외하고 실제 Session 필드는 전부 비교한다. 새 active 재시도의 두 번째 클릭 거절도 추가한다.

## RETRY-02 — 집중 회귀와 freeze

- 예정 명령: `cargo test --test conversation_retry --test workflow --test state_reset --test shared_settings -- --nocapture`, `cargo clippy --lib --test conversation_retry --test workflow --test state_reset --test shared_settings -- -D warnings`, `cargo fmt --check`.

- 최종 결과: conversation_retry4 + shared_settings5 + state_reset7 + workflow7 = **23 pass / 0 fail**, exit0. retry4의 내부 표는 no-run/completed/design-failed/critic-failed/pending-termination/unknown-termination/다른 세션 active 거절과 cancelled·manual recovery 허용을 포함한다. 새 active 재시도 중 두 번째 명시 클릭도 run_busy로 거절했다. 소유 소스와 테스트 rustfmt, focused clippy `-D warnings`, `cargo fmt --check` 모두 exit0.
- 실제 모델 호출·사용자 프로젝트/DB/서버 변경은 없었다. 합성 pending worker는 missing executable에서 정상 실패 정리되었고 기존 Session 메시지·coverage·claims·requirements·decisions·초안·design_revision 전체를 보존했다. root에 API/source freeze를 통지한다. UI 버튼과 Domain 한도 변경은 해당 작성자의 별도 검증 대상이다.
