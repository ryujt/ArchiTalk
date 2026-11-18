# 프로젝트 기록 격리와 공통 AI 설정 — 2026-11-18

## 요구와 범위

- 사용자 요청: 프로젝트 `.architalk`를 삭제해도 이전 내용이 보이는 현상 수정, 프로젝트 기록은 해당 폴더에 한정, 지금 기록 삭제. 후속 정정에 따라 에이전트·모델·생각 수준 등 공통 AI 설정은 공용 설정 파일에 저장한다.
- 확인: 제품에는 공용 대화 DB가 없으나 실행 중 App의 메모리·pinned SQLite fd와 browser state가 삭제 뒤에도 살아 있다. 실제 지정 프로젝트 DB에 screenshot과 같은 2 sessions/2 failed runs가 있었고, 모델은 버전 검사에서 실행되지 않았다. 현재 대상 DB를 연 서버는 없었다.
- 사용자 지정 프로젝트의 `.architalk`를 root inode lock 확인 후 삭제했다. 공통 ArchiTalk 설정 폴더는 당시 존재하지 않았다. 별도로 Claude가 해당 project cwd로 직접 남긴 2 sessions/그 session-env 및 history index 3건도 확인해 삭제했다. 다른 cwd의 Codex session에 있는 단순 경로 언급은 해당 프로젝트 세션으로 간주하지 않고 유지했다. 인증·다른 프로젝트 자료는 삭제하지 않았다.
- 변경 소유: root 공용 설정/main/README/통합·실제 정리, Storage 담당 삭제 감지/HTTP410/commit gate, UI 담당 권위 snapshot/reset 화면/브라우저, AI 담당 명시 327 profile·project runtime·쓰기 격리. 각 담당 테스트 기록은 별도 파일이다.

## SHARED-01 — 공통 설정 계약 검증 계획

- 공통 파일에는 AI 일반 설정만 쓰고 project output directory·대화·run·source path는 기록하지 않는다. 기존 generic document defaults는 보존한다. 파일 lock/CAS와 atomic rename으로 다른 프로젝트의 동시 설정 변경을 덮어쓰지 않는다.
- product main은 명시한 공통 config path를 App에 전달한다. 기존 `App::open`은 library/fixture의 격리 모드를 유지하여 실제 사용자 설정을 테스트가 건드리지 않게 한다.
- 검증: 서로 다른 두 합성 project가 공통 AI 설정을 읽고 다음 run에 사용, 문서 경로는 각 DB, 재시작 후 최신 공통 AI 반영, stale settings PATCH 거절, 같은 목표값 retry 멱등, 파일에 프로젝트 이름/내용/경로 없음. 임시 디렉터리만 사용하며 일반 테스트는 실제 모델을 호출하지 않는다.
- 실행 전 계획: 새 `tests/shared_settings.rs`, 기존 settings/workflow·Node·Rust 정적 검사, 최종 browser reset/common settings 소비자 확인. 실제 Claude 327 smoke는 runtime 격리 통합 후 root가 별도 1회만 합성 입력으로 수행하고 기록한다.

- SHARED-01 최초 실행: `cargo test --test shared_settings --test settings_choices --test workflow`. 공통 설정 3/3·선택 metadata2/2 통과. workflow6/7 통과, critique preparation fixture 재시작에서 project_locked가 관측되어 원인 조사 중이다. 파일/설정 계약 통과와 이 조립 실패를 구분하며 같은 조건의 무근거 반복은 하지 않는다.

## ISO-REAL-01 — Claude 327 실제 요청 계획

- runtime/버전/삭제 가드의 fixture 통과 뒤 `cargo test --test real_project_isolation -- --ignored --nocapture`를 명시적으로 한 번 실행한다. 임시 합성 프로젝트에서 `{ok:true}`만 요청하고 실제 프로젝트 내용은 보내지 않는다. 저사고 설정·90초 제한이며 모델 사용량이 발생한다.
- 기준: 실제 설치된 2.1.327이 terminal JSON을 반환하고, run-only 파일쓰기 sandbox 안에서 정상 동작하며, 프로젝트 `.architalk/runtime`의 실행 폴더가 완료 뒤 정리된다. 공용 인증 파일은 읽기 재사용한다. quota 없는 OS 차단 fixture와 실제 동작을 구분한다.
- 일반 Rust suite에서는 ignored이며 자동 재시도는 없다. 실패하면 오류와 원인을 보존하고 같은 실제 요청을 무근거 반복하지 않는다.

- ISO-REAL-01 결과: **1 pass / 0 fail**, 실제 요청 2.09초, CLI 2.1.327, terminal JSON 일치·run-only write sandbox·private runtime 정리가 통과했다. CLI가 보고한 resolved model은 claude-opus-5-5였다. 인증 자료 복사나 공용 기록 쓰기 허용 확대 없이 정상 동작했다.

## ISO-REAL-02 — Codex 동일 실행 경계 계획

- 쓰기 격리와 Codex SQLite/log 위치도 변경됐으므로 동일한 작은 합성 JSON 요청을 Codex 0.169.0에 한 번 명시적으로 실행한다. low·90초 제한, 실제 프로젝트 자료 없음. fixture의 OS 차단과 실제 프로토콜/인증·runtime cleanup을 함께 확인한다. 추가 설계 문서 생성은 하지 않는다.

- ISO-REAL-02 최초 결과: **0 pass / 1 fail**, 7.12초, terminal 응답 전 `cli_failed`가 발생했다. 현재 adapter가 stderr 원인을 일반 메시지로 감추므로 공유 경로/실행 인자의 어느 부분인지 아직 확정하지 않는다. 실제 JSON 요청은 1회이며 추가 요청 없이 AI 담당이 진단·원인 분류를 보강한다. Claude 통과를 Codex 통과로 일반화하지 않는다.

- SHARED-02 실행: 다음 run 직전의 다른 프로젝트 공통 설정 변경까지 확인하는 4번째 테스트를 추가한 `cargo test --test shared_settings`는 **4/4 통과**했다. CLI를 일부러 없는 실행 파일로 지정해 모델 사용 없이 새 run의 model/timeout snapshot이 최신 공통 값인지 확인했다. 이후 서버가 꺼진 동안 global AI가 바뀌면 재시작 시 이전 form revision도 무효화하도록 보완했으며, 이 5번째 반례는 최종 suite에 포함한다.
- 공용 설정 비작성 검토는 Typed AI만 저장·project docs 보존·hash/lock CAS·다음작업source를 확인해 중요 blocker가 없었다. 공통 파일과 프로젝트 DB는 하나의 원자 transaction은 아니며, 파일 반영 뒤 DB 실패가 발생해도 다음 GET이 실제 공통 파일을 기준으로 표시한다. 프로젝트 원문을 공통 파일에 기록하지 않는다.

- SHARED-03: `cargo test --test shared_settings` **5/5 통과**, 0.11초. 공용 변경을 서버 재시작 중에도 이전 form revision으로 덮어쓸 수 없게 했고, next-run model/timeout·문서경로 분리·멱등 CAS와 함께 확인했다.
- Codex 추가 조사: 공용 HOME를 read-only로 두면 공식 0.169.0 in-process server가 installation_id를 항상 RW/create로 열어 EPERM이 발생한다. network-denied 대조에서 run-local CODEX_HOME은 그 지점을 지나갔다. 인증은 native read-only file link로 재사용하고 runtime home 전체를 local로 옮기는 수정 뒤 ISO-REAL-03을 같은 작은 JSON으로 명시적으로 1회 재검증할 계획이다. 권한을 공유 홈 전체로 확대하거나 인증 원문을 복사하지 않는다.

## FINAL-01 — 최종 조립 회귀 계획

- AI/local CODEX_HOME 원인 수정·Storage/App/HTTP/reset·UI source 동결 뒤 fmt/clippy all-targets 및 전체 Rust suite를 실행한다. CLI metadata는 project runtime에만 두고 공용 설정 파일의 model/agent 값만 공유한다.
- Node 최종 조합은 `node --test tests/web.test.mjs tests/design-package-ui.test.mjs` **33/33 통과**(417ms). browser 기존 전체16/optional2skip, 마지막 reset badge 소비자2건도 별도 통과했다.
- Root 실제 Claude327 한 번과 Codex 최초 실패/원인 수정 후 한 번을 구분한다. 추가 대형 설계 생성이나 모델 자동 재시도는 없다. 실패 원인과 기존 테스트 실패 기록은 삭제하지 않는다.

- ISO-REAL-03 결과: 원인 수정 뒤 Codex 0.169.0 동일 tiny JSON **1 pass / 0 fail**, 실제 요청5.21초. run-only sandbox·terminal JSON·runtime 정리가 통과했다. CLI가 resolved model을 제공하지 않아 특정 모델명을 추정하지 않는다. 최초 실패는 ISO-REAL-02에 유지한다. 실제 Claude 성공1회, Codex 최초 startup실패1회+수정후성공1회, 이외 진단은 네트워크 차단·모델 호출0으로 구분한다.

- FINAL-01 최초 전체 suite: fmt/clippy 통과, document_format 10/11에서 기존 invalid-design fixture의 외부 attempts 파일 쓰기가 새 run-only 경계에 차단돼 cli_failed로 끝났다. invalid_model_output까지 도달하지 못한 **테스트 fixture 적합성 오류**다. 제품 sandbox나 원래 오류/기존 초안 보존 단언은 완화하지 않는다.
- 수정: CLI가 외부 counter 파일을 쓰지 않고 parent가 실제 Claude initialization progress를 구독해 모델 generation 횟수를 센다. fixture는 매 generation마다 해당 verified init을 정확히 한 번 보내므로 자동 보정/재호출이면 두 번으로 검출된다. one run/invalid output/기존 draft byte 보존 단언을 유지한다. focused 재검증 뒤 같은 전체 suite를 재실행한다.

- FINAL-01 두 번째 전체 suite: invalid-design fixture 수정 후 통과했으나 storage_spike의 초기 실험 1건이 이름 교체 뒤 old inode에 commit 성공을 기대해 새 reset 계약과 충돌했다(2pass/1fail). 외부 symlink 미쓰기 기준은 유지하고, 이제 typed WorkspaceResetRequired·old DB bytes 불변·old snapshot 조회 거절을 함께 확인하도록 기대를 갱신한다. 새 계약이 과거 실험의 loose behavior를 강화한 것으로 구분하며 제품 guard는 완화하지 않는다.

## FINAL-01 최종 결과

- 전체 `cargo test --locked` **129 pass / 10 ignored / 0 fail**. 최종 ignored에는 실제 모델 smoke/전체 workflow 및 명시적 network-denied bootstrap 진단이 포함되며 별도 실행 결과와 구분한다. 앞선 두 전체 실행의 fixture/옛 계약 실패와 최초 lock 실패는 위 기록 및 담당 기록에 보존했다.
- Node 조합33건 통과. 실제 browser 전체16건/opt-in2skip, UI terminal badge 수정 후 관련2건 재검증 통과. AI source의 후속 변경은 native Codex home/error 분류이며 UI·상태 API는 바꾸지 않았고 Rust adapter/실제 Codex smoke로 소비자를 확인했다.
- 실제 설치 CLI의 합성 tiny 요청은 Claude2.1.327 2.09초와 Codex0.169.0 수정 후5.21초에 정상 terminal JSON·run-only쓰기·scratch정리를 통과했다. Codex 최초 startup 실패와 추가 모델 요청 없는 진단/원인 수정은 별도 기록이다. 이번에는 대형 설계 패키지나 실제 전체 conversation→critic→save 유료 workflow를 다시 실행하지 않았다.
- 최종 정리 재확인에서 사용자가 지정한 프로젝트는 빈 폴더이고, 해당 cwd의 Claude project history도 없다. 공용 인증 파일과 다른 cwd의 기록은 유지했다. common config가 없으면 기본값을 쓰며, 명시적으로 저장한 agent/model 값은 공용 TOML에서 다른 프로젝트에 공유한다.
- source 변경 완료 후 release 설치·프로젝트 밖 version 확인 및 커밋/푸시를 진행한다. Linux/Windows·전원 손실·장기 부하·실제disk-full은 이번 검증 범위가 아니다.

## 설치·완료

- 최종 fmt/clippy all-targets `-D warnings` 재검증 exit0. release 설치 `./install.sh` exit0, 7.95초, 전역 0.2.0→**0.2.1** 교체 완료. `/tmp`의 새 로그인 셸에서 Cargo bin 경로와 `architalk 0.2.1`을 확인했다.
- README·implementation·검증 색인의 모든 상대 링크가 존재한다. 완료 직전 지정 프로젝트 폴더가 빈 상태임을 다시 확인했고, 실제 사용자 브라우저 탭이나 다른 프로젝트 실행을 조작하지 않았다. 새 서버/새 접속 URL로 재시작해야 기존 실행의 UI 자산도 교체된다.
- 책임/검토: root 공용 AI 파일·조립·정리/실물 CLI·최종회귀, Storage 담당 identity guard/HTTP/SSE/취소/락, UI 담당 stale/terminal/client 연결 및 browser, AI 담당 명시327지원/private runtime/Codex home/auth 경계. 공용 설정은 비작성 Storage 관점으로 검토했고, root는 조합 diff와 새 계약을 검증했다. 동일 모델 계열을 상속해 역할을 나눴으며 미확인 사용량/비용은 추정하지 않는다.
