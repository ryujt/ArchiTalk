# 대화 스크롤·인라인 진행 구현 검증

## 범위와 이전 기록

- 기준: `main@5954230` 작업 트리. 제품 쓰기 소유권은 `web/app.js`, `web/style.css`, 이 기록이다. `src/ai.rs`의 한국어 진행 안내와 공용 테스트·색인은 다른 담당자가 소유한다. 커밋과 실제 유료 모델 호출은 하지 않는다.
- 이전 기록: [implementation-web-browser](implementation-web-browser.md) B04/B05의 같은 세션 snapshot 경합과 실제 desktop/mobile 통과, [model-choices-browser-20261118](model-choices-browser-20261118.md) B01/R01의 설정 모달·Node 21개 회귀 계약을 참고했다. 기존 경합 방어와 설정 입력 보존 경로를 유지하며 이번 UI 변경의 회귀를 확인한다.
- 원인: 긴 interview를 짧은 generating status-card로 교체하면서 document 스크롤이 짧아지고, 완료 뒤 최신 답변·입력창을 다시 보여주지 않는다. 전체 화면 끝은 긴 보조 패널의 끝일 수 있으므로 대화의 composer 하단을 기준으로 삼는다.
- 변경: 기존 interview/review 형태를 유지하며 assistant 진행 버블과 중지 버튼을 추가한다. 원시 CLI JSON은 표시하지 않는다. 새 대화/새 메시지/새 작업에만 하단을 따라가며 같은 snapshot과 모달 갱신은 위치를 보존한다. 대화 완료는 기존 초안 유무와 관계없이 interview로 돌아온다. interview 하단에 primary 다음 단계 CTA를 두고 중앙 콘텐츠의 760px 폭 제한을 제거한다.
- 독립 역할: root가 통합, 별도 Tester가 수정 전 desktop/mobile 실패와 수정 후 실제 브라우저를 확인하고 별도 Critic이 반례를 검토한다. 이 담당자는 구현과 Node 회귀를 수행한다. 모델은 부모에서 상속했고 별도 모델을 지정하지 않았다.
- 실제 브라우저 실행은 Tester의 별도 기록에 남긴다. 이 기록의 Node 검사는 실제 레이아웃·스크롤을 검증했다고 주장하지 않는다.

## 실행 기록

### S01 — 제품 JavaScript 구문·공백

- 시각: 2026-11-18 08:26 KST, 실행 전 항목 작성·종료 관찰 08:26:52 KST. 개별 명령 시작 초는 미측정. 기준: 위 변경을 적용한 작업 트리.
- 명령: `node --check web/app.js`, `git diff --check -- web/app.js web/style.css tests/test-history/chat-scroll-implementation-20261118.md`.
- 결과: 두 명령 모두 exit 0. 실행 도구의 각 벽시계 표시는 약 0.1초이며 정확한 실행 시간은 별도 측정하지 않았다.

### N01 — 기존 프로토콜·상태 회귀

- 시각: 2026-11-18 08:26 KST, 실행 전 항목 작성·종료 관찰 08:26:52 KST. 개별 명령 시작 초는 미측정. 기준: S01과 동일한 제품 작업 트리.
- 명령: `node --test tests/web.test.mjs`.
- 결과: exit 0, **21개 통과·실패/건너뜀 0개**, Node 보고 약 **377ms**. 실제 브라우저가 없는 가짜 DOM은 API/상태/동시성 회귀에 한정한다. 기존 B04의 지연 snapshot·session/run 사이 완료 회귀 두 개도 통과했다. 실제 레이아웃 검증은 아래 별도 실행에 연결한다.

## 독립 검토·브라우저 재검증 연결

- 별도 Critic의 최종 읽기 전용 diff 검토에서 수정이 필요한 결함은 발견하지 못했다. 작업 시작 화면 보존, completed conversation 복귀, 메시지 fingerprint와 모달 보류, 긴 보조 패널 대신 composer 기준 이동, 원시 JSON 비노출, 기존 generate API/disabled 경계, 중앙 폭 제한 제거를 확인했다. 실제 geometry 판정은 브라우저 담당자의 결과로 구분했다.
- 별도 Tester는 [chat-scroll-browser-20261118](chat-scroll-browser-20261118.md) B01의 수정 전 desktop/mobile **2건 실패**를 보존했다. 같은 파일의 B02에서 수정 후 신규 대화 시나리오 4건과 기존 설정/워크플로 4건, **총 8건 통과**를 확인했다. 실제 3턴 최신 답변·입력창·CTA, 과거 읽기 snapshot 위치, 인라인 진행·중지, 기존 초안 이후 대화, 1920px의 가용폭 채우기를 검증했다. 이 담당자가 그 브라우저 명령을 중복 실행하지 않았다.
- 잔여 한계: 실제 Claude/Codex 유료 호출과 모델 응답 품질은 이번 검증 범위가 아니다. 브라우저 캡처·geometry·자원 정리는 브라우저 기록의 실행 ID와 보존 정책을 따른다.
