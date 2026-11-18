# 프로젝트 기록 초기화와 브라우저 상태 검증

## 사전 범위

- 기준: `079042b`. 사용자 요구는 agent/model 설정은 공용 유지하고 대화·설계·작업 등 프로젝트 기록을 해당 프로젝트 `.architalk`에만 두는 것이다. 기존 사용자 탭·공유 설정·DB·브라우저 storage는 조작하지 않는다. root가 기존 기록 정리·공용 설정과 설치를, 다른 담당자가 storage/HTTP/runtime guard를 소유한다.
- browser 계약: sessionStorage에는 capability와 마지막 session ID만 있었으며 대화 본문은 저장하지 않는다. API fetch는 no-store다. 다만 authoritative workspace에 현재 session이 없어도 기존 state.session이 있으면 clear branch가 실행되지 않아 old message/run/feed가 남았다. 401도 오류 안내만 하고 본문을 남겼다.
- 읽기 전용 합성 Node probe는 workspace sessions=0인데 session-1/interview/old text/feed가 그대로인 반례를 확인했다(exit0). 사용자 데이터나 모델 호출은 없었다.
- 소유: `web/app.js`, `tests/web.test.mjs`, `tests/browser/project-reset.spec.mjs`, 이 UI 기록. backend guard의 RST 기록은 별도 [project-state-reset](project-state-reset-20261118.md) 담당자 소유다. 합의 후 N01~N04를 이 파일로 한 번 이동했고 RST 본문은 보존했다.
- 신규 Node 반례는 empty snapshot, terminal reset SSE+late command/delta, memory-only last selection, 401 무효 연결이다. storage/HTTP 계약은410 + `workspace_reset_required`; 열린 SSE는 동일 이름/type event 한 번 후 종료다. 일반 연결 끊김은 기록을 지우지 않고 기존 reconnect 계약을 유지한다.

## 실행 기록

### N01 — 수정 전 반례

- 명령: `node --test --test-name-pattern='authoritative empty|terminal reset|last-view selection|revoked connection' tests/web.test.mjs`.
- 제품 수정 전 네 반례를 실제 browser 모듈의 기존 synthetic Node harness로 확인한다. 모델/HTTP 서버/사용자 브라우저/DB를 호출하지 않는다. 실패를 보존하고 권위 empty/terminal reset 경계만 수정한다.

- N01 결과: **4 fail / 0 pass**, exit 1, 약720ms. authoritative empty인데 old session이 남고, terminal SSE가 block 상태로 바뀌지 않으며, legacy selection ID를 복구하고, 401에 stale view를 비우지 않는 네 반례를 제품 수정 전 확인했다.

### N02 / N03 — 선택/연결 경계 수정과 Node 회귀

- 변경: 선택 session ID는 메모리에만 보관하고 obsolete sessionStorage key를 bootstrap에서 제거한다. capability는 정상 reload용 sessionStorage를 유지한다. authoritative workspace에서 현재 session이 빠지면 project view/run/draft/plan/cache/feed를 비운다. 410 reset code 또는401에는 token/old view/modal을 제거하고 SSE·timer·새 명령을 막는다. 별도 workspace epoch로 늦은 command 결과를 무시한다. 일반 일시 연결 장애는 기존 reconnect 동작을 유지한다.
- `node --check web/app.js` exit 0. 수정 전 네 반례 표적 재실행은 **4 pass / 0 fail**, 약175ms, exit 0이었다. `node --test tests/web.test.mjs`는 전체 **26 pass / 0 fail**, 약408ms였다.
- 접근성/표시 보강 후 같은 전체 Node 회귀도 **26 pass**, 약429ms였다. terminal 상태에서 기존 modal DOM을 지우고 controls를 비활성화하며 새 주소 안내를 announce/focus한다. AI/model 등의 공용 설정을 삭제하지 않는다.

### N04 — 동일 origin 새 URL/공용 설정 범위

- 추가 발견: 고정 port에서 새 접속 주소가 token fragment만 바꾸면 브라우저는 새 JS bootstrap 없이 hashchange만 실행한다. token fragment가 들어오면 이전 view/연결을 닫고 실제 reload하도록 보강했다. 일반 문서 anchor는 그대로다. 명시적으로 빈 token fragment는 오래된 token으로 fallback하지 않는다.
- Node 반례를 추가하고 `node --check web/app.js`, `node --test tests/web.test.mjs`를 실행했다. **27 pass / 0 fail**, 약419ms, exit 0. memory-only selection/terminal reset과 함께 공용 AI 설정 browser copy가 유지되는 것도 확인했다.
- 브라우저 신규 spec 구문 검사 exit 0. 실제 사용자 브라우저나 DB를 건드리지 않고 격리된 테스트 프로젝트의 `.architalk`만 삭제한다. 생성된 초안/모달을 열고 fixture의 5초 지연 요청이 active인 상태에서 삭제하여 관찰할 여유를 확보한다. 동일 origin token fragment의 실제 document reload도 확인한다. backend/UI source 동결 뒤 표적·전체 회귀를 시작한다.

### B01 — 실제 삭제·SSE·동일 origin 새 접속 표적

- root가 shared config/Store/App/HTTP/UI source를 동결했다. `cargo build --bin architalk` **6.36초**, exit0 후 0.2.1 binary를 run 소유 `target/browser-binaries/project-state-reset-20261118/architalk`로 복사했다. SHA-256 **629be46bde0f0c4f703eb195e711171a4fa694e48ca9bf8ec4c9912400d1e7c5**, 크기 **33,372,312 bytes**다. root의 병렬 cargo build가 다음 browser case의 제품을 바꾸지 않는다.
- 명령: `ARCHITALK_BROWSER_BINARY=<위 고정 복사본> ARCHITALK_BROWSER_RUN=project-state-reset-targeted-20261118 npx playwright test tests/browser/project-reset.spec.mjs`.
- desktop/mobile에서 실제 fixture conversation/design/critic로 rich draft를 만든다. 같은 origin token fragment가 실제 HTML reload를 일으키는지 확인하고, 5초 fixture conversation이 active인 동안 old draft/settings modal을 열어 둔 채 **이번 서버 소유 임시 프로젝트** `.architalk`만 삭제한다. terminal 안내·닫힌/비워진 modal·old content/ID/auth 부재·더 이상 online polling 없음·API410·디렉터리 부활 없음·가로 넘침/pageerror/외부 요청 경계를 판정한다. 실제 사용자 DB/탭/provider home/공용 설정 삭제와 모델 호출은 없다.

- B01 결과: desktop/mobile **2 fail**, exit1. token fragment가 실제 reload를 시작한 것은 확인했으나 테스트가 response header 직후 새 document commit 전에 evaluate하여 `Execution context was destroyed`가 났다. 디렉터리 삭제 단계에는 도달하지 않았다. **테스트 navigation barrier 결함**으로 분류한다.
- 수정: fragment를 바꾸기 전에 다음 `DOMContentLoaded` event를 등록하고 HTML response와 함께 기다린 뒤 marker 소멸을 확인한다. 실제 reload/기록 복구 assertion을 유지했으며 임의 sleep·exception 무시·재시도는 추가하지 않았다. 신규 spec 구문 검사 exit0.

### B02 — 실제 document load barrier 후 표적 재검증

- 명령: `ARCHITALK_BROWSER_BINARY=<같은629be46b... 복사본> ARCHITALK_BROWSER_RUN=project-state-reset-targeted-r2-20261118 npx playwright test tests/browser/project-reset.spec.mjs`.
- 제품 binary는 B01과 같고 테스트 barrier만 바로잡았다. 두 viewport의 실제 reset까지 다시 검증한다.

- B02 결과: desktop/mobile **2 pass / 0 fail**, exit0, Playwright **6.6초**, retries0. 같은 origin token fragment에서 실제 HTML reload/새 Window marker를 확인했다. 5초 지연 fixture Run이 active임을 삭제 직전 API로 확인하고 임시 `.architalk`를 삭제했다. rich preview/모달/old message/선택 ID/서버 capability가 제거되고 controls가 비활성화됐으며, 다시 online event를 보내도 새 API 요청이 없었다. 서버410 reset code와 `.architalk` 비재생성도 통과했다. 사용자 프로젝트·공용 provider 설정·사용자 탭은 조작하지 않았다.

### B03 — 독립 전체 브라우저 회귀

- 명령: `ARCHITALK_BROWSER_BINARY=<같은629be46b... 고정 복사본> ARCHITALK_BROWSER_RUN=project-state-reset-final-20261118 npx playwright test`.
- 범위: 기존14개 browser scenario + 실제 reset2개 =16개, 실제 보존 응답 opt-in 2개는 ENV 미지정으로 skip한다. 같은 고정 binary로 package/10파일 저장·보안/CSP 없는 대조·설정/진행중 Run·대화 scroll/pending·recovery를 함께 검사한다. root의 이후 Codex error 분류 변경은 성공 Claude fixture와 UI 소비자 계약에 영향이 없다는 범위로 구분한다.

- B03 결과: **16 pass / 0 fail + 2 opt-in skip**, exit0, Playwright **1.1분**. 모든 기존 package/파일 저장·보안·설정/다음 Run·대화/scroll/pending·recovery와 신규 실제 reset 경계를 같은 고정 binary에서 통과했다. opt-in skip은 실제 응답 경로 ENV가 없어서이며 실제 생성 검증 통과로 합산하지 않는다. PNG62장과 reset 경계 JSON을 보존했다.

### B04 — terminal 연결 배지 보정 후 관련 재검증

- 시각 QA에서 실제 새 API 요청은 0인데 늦은 refresh catch가 연결 배지 문구만 `재연결 · 상태 확인 중`으로 바꿀 수 있음을 발견했다. terminal connection은 `새 접속 필요`로 고정하여 안내와 상태가 모순되지 않게 했다. 정상 연결/reconnect 동작은 바꾸지 않았다.
- Node 전체에 terminal 배지 판정을 추가한 뒤 **27 pass / 0 fail**, exit0, 약409ms였다. browser에도 정확한 배지 text 판정을 추가했다.
- `cargo build --bin architalk` **4.75초**, exit0. 최신 run 복사본 `target/browser-binaries/project-state-ui-final-20261118/architalk`, SHA-256 **8befaee8f1694ad033da92b00711d683ec5f83a83eb4a7eab16bae9e0eb79292**, 크기 **33,374,072 bytes**를 고정했다.
- 명령: `ARCHITALK_BROWSER_BINARY=<위 최신 복사본> ARCHITALK_BROWSER_RUN=project-state-reset-ui-final-20261118 npx playwright test tests/browser/project-reset.spec.mjs`.
- 결과: desktop/mobile **2 pass / 0 fail**, exit0, Playwright **8.0초**, retries0. terminal badge까지 포함한 reset 관련 case만 다시 검사했으며 이미 통과한 전체16개를 이유 없이 반복하지 않았다.
- 실제 최종 수치: 두 viewport 모두 `sameOriginCapabilityReboot=true`, `activeRunAtRemoval=true`, old content/선택/auth 부재, API410, 디렉터리 부재를 확인했다. reset 직후/online event 뒤 browser API 요청 수는 desktop **36/36**, mobile **35/35**로 같았다. pageerror·외부 요청·Bearer 경계 위반은0이다.
- 시각 확인: B02 desktop/mobile terminal PNG를 실제로 열어 이전 대화·초안·모달이 보이지 않는 것을 확인했다. B04 desktop terminal PNG도 실제 열어 최종 배지 `새 접속 필요`, 비활성 controls와 서버 재시작/새 접속 주소 안내를 확인했다. 정상 HTML reload의 context 파괴와 B01 테스트 barrier 실패도 원본으로 보존했다.
- 최종 tracked source/test `git diff --check` exit0. 사용자 탭·공유 provider home·사용자 DB는 조작하지 않았다. 삭제 주입은 이번 실행 소유 임시 프로젝트만 대상으로 했다. UI source 및 이력은 동결해 root에게 전달하며, commit/설치/기존 실제 기록 정리는 root 소유다.
