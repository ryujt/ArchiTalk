# Codex 진행 안내 문구

- 기준: `5954230`의 깨끗한 작업 트리. 사용자가 Code Mode disabled 안내를 문의했다.
- 확인: `src/ai.rs`의 알려진 Codex startup diagnostic을 일반 Progress로 변환하는 기존 분기에서 ArchiTalk가 생성한 문구다. Code Mode를 끄는 호출 옵션과 텍스트 응답 완료 판정은 기존 동작이다.
- 변경: 사용자 화면의 이 진행 문구만 `Codex가 답변을 생성하고 있어요.`로 바꾼다. 다른 오류는 기존처럼 실패로 보고하며 완료 이벤트와 본문 없이는 성공 처리하지 않는다.
- 이전 이력: [implementation-ai](implementation-ai.md) AI-P1의 실제 Codex text turn 완료 근거 및 [model-choices-backend](model-choices-backend-20261118.md)의 최신 adapter 검증을 참고했다.

## CP-01 — 기존 startup 진단 계약

- 상태: 2026-11-18 08:21:51 KST 시작, 약 2초 후 통과. `cargo test --lib codex_known_startup_notice_is_not_terminal_success` 1건 통과, 이어 `cargo fmt --check` 통과. 안내가 terminal 성공으로 오인되지 않는 기존 경계를 확인했다. 새 반복 테스트는 추가하지 않았다.
- 별도 Critic은 같은 분기의 한 줄 문구 변경을 읽기 전용 검토한다. 실행은 통합 담당자가 맡고 상속 모델 설정을 유지한다.
- 독립 검토 결과: 해당 분기는 Progress만 반환하고 상태를 바꾸지 않으며, 일반 오류는 계속 실패 처리한다. 필수 지적 없음.

## CP-02 — 최신 전역 실행 파일 반영

- 상태: `./install.sh`가 release 빌드 약 6.9초 후 전역 실행 파일을 성공적으로 교체했다. 실제 모델 호출·사용자 실행 프로세스 종료는 하지 않았다. 이후 같은 턴의 채팅 스크롤 개선 요청이 추가되어 최종 설치·커밋은 해당 변경과 함께 진행한다.
