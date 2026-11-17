# TO-BE 문서·화면 검증

기준: `main` / `d5fa5d0`에서 새 문서·화면 모형과 README를 작성한 작업 트리. 시각은 KST. 최초 검증으로 이전 이력 없음. 기록 소유자는 통합 담당이다.

## R01 — 독립 설계 검토

- 시각: 정확한 검토 시작/종료 시각은 미측정. 작업 생성 시각 2026-11-17 19:00:55 KST 이후 수행했으며 19:18 KST에 최종 재검토 결과를 기록했다.
- 방법: 프로세스 조사 담당과 기술 구조 담당이 통합 본문을 독립 검토. 실행 테스트 없이 반례·계약을 대조.
- 결과: 1차 필수 지적 9개(프로세스 4, 기술 5), 재검토에서 필수 명료화 3개(문서/동시성 revision 분리·semantic revision 증가 조건·재시작 종료 미확인 차단)를 발견했다. 모두 반영 후 담당자가 변경된 계약을 재확인했다. 권장사항도 반영했다.
- 처리: 보류 게이트·근거 해석·수락/저장·초안 최신성, 프로젝트 밖 CLI cwd, immutable no-clobber 출력, 상태 경로 보호, 임의 args 금지, 취소 slot 보존을 수정했다. fixture oracle·receipt 순서·session feed·무출력 처리·CLI 기본값 의미도 보완했다.
- 한계: 모델 실제 호출·제품 실행·OS sandbox·no-clobber 구현 검증 아님. 본문 V01~V13은 향후 제품 검증 계획.
- 다음: UI 변경 반영 후 정적 링크/장면 검증. 실제 제품의 CLI 격리·OS별 저장/종료 보장은 구현 P0에서 확인.

## R02 — 정적 문서·참조 검사

- 상태: 통과. 최종 UI 변경 반영 후 재대조 완료.
- 대상: `docs/design/20261117-190055/to-be/`, README, 테스트 이력.
- 판정: 로컬 파일/fragment 링크 유효, 코드 펜스 짝, 페이지·장면의 Navigation/Layout/HTML 대응, 금지된 작성 컴퓨터 절대 경로 없음, JS 구문 오류 없음, diff whitespace 오류 없음.
- 1차 실행: 2026-11-17 19:18 KST, 프로젝트 루트에서 `python3 - <<'PY'`로 Path/re/HTML heading slug 검사. Markdown 9개, 로컬 링크 102개, 닫힌 펜스 확인. jobflow 8개에서 header/Object/관계·미선언 객체를 검사했으며 오류 0개. machine-specific 절대 경로 0개. 검사 범위는 파일 존재·heading/explicit anchor·기본 DSL 구조이며 전용 parser/renderer 검증이 아니다.
- UI의 독립 실행은 [UI-S01/S02](ui-20261117-static.md)에서 확인한다. 페이지 1개·HTML 1개·17장면·8과업 대응, JS 구문, 생성 마크업, 모름 처리, 미생성/과거 초안, 불변 문서 snapshot, 개정 폴더, 실제 네트워크 호출 없음이 통과했다.
- 최종 재대조: 2026-11-17 **19:20:31~19:20:31 KST**, 같은 Python 방식으로 Markdown 9개·로컬 링크 104개·Job Flow 8개·Navigation 7개·Layout 10개·Mermaid 1개·TOML 1개의 펜스를 확인했다. 경로/앵커/기본 Job Flow 구조 오류 0개. Navigation 과업 8개 중 N02는 같은 화면 동작을 본문으로 설명해 전용 블록은 7개다.

## R03 — 화면 모형 브라우저 검토

- 상태: 차단·미실행.
- 판정: desktop/mobile의 대화·설정·검토·초안/검토본 저장·충돌·복구 장면과 직접 fragment 링크 확인. 실제 API/CLI를 호출하지 않는 모형임을 확인.
- 실행: 2026-11-17 19:17~19:18 KST, CUA의 `createBrowserTab`으로 로컬 `workspace.html` 열기를 시도했으나 `The browser access host is unavailable`을 반환했다. 탭/화면 결과를 관찰하지 못했다.
- 실제 결과: 브라우저 시나리오 0건 실행. desktop/mobile 렌더링·dialog 포커스·폭별 배치·키보드 동작은 확인하지 못했다. 별도의 정적 가짜 DOM 검증을 브라우저 성공으로 대체하지 않는다.
- 후속: 브라우저 접근 환경에서 17개 직접 fragment, N01~N08, desktop/mobile 폭, 입력·모달·복귀 동작을 확인해야 한다. 문서 작성 결과는 제공하되 시각 QA 완료로 보고하지 않는다.

## 미실행 범위

Rust 제품 코드·실제 모델 호출·로그인·CLI 권한 격리·OS 프로세스 종료·DB/파일 crash 복구·실제 사용자 효과 평가는 구현 전이다. jobflow/navigation/layout/Mermaid 도표는 별도 렌더러를 실행하지 않는 한 정적 검토로만 판정한다.

## R04 — 커밋 전 변경 검사

- 상태: 통과. UI 파일 쓰기 동결 후 실행.
- 이전: R01 필수 계약 수정·재검토, R02 링크 확인, UI-S02 최종 정적 통과를 참고한다. 브라우저 차단을 통과로 바꾸지 않는다.
- 방법: 최종 파일의 링크·펜스·Job Flow 기본 구조를 재대조하고, 명시 경로만 stage한 뒤 `git diff --cached --check`와 staged 파일 목록을 확인한다.
- 실제 명령: `git add README.md docs/design/20261117-190055/to-be tests/test-history`, `git diff --cached --check`, `git diff --cached --stat`, `git diff --cached --name-only`.
- 결과: 2026-11-17 19:20:31 KST, 각 명령 exit 0. whitespace 오류 0, README·설계 문서·HTML 모형·검증 이력만 10파일 포함. 제품 소스 변경 없음. 결과 기록 수정 후 같은 whitespace 검사를 최종 커밋 직전에 한 번 더 수행한다.
