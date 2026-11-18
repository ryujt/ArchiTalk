# 브라우저 복사 실패의 로컬 대체 경로 검증

## 사전 범위와 계약

- 기준은 `591096a`(0.6.0)이며 사용자에게 보인 “자동 복사를 사용할 수 없어요”는 기존 browser-only 복사의 포괄 오류다. 이번 변경은 브라우저 직접 복사 후 인증된 로컬 `POST /api/v1/clipboard`로 대체하고 두 경로가 모두 실패한 경우 원인 안내와 수동 복사를 제공한다.
- 담당 쓰기 범위는 `tests/browser/document-voice.spec.mjs`, `tests/browser/viewer-workspace.spec.mjs`, 이 실행 이력이다. UI/서버 구현·README/배포는 다른 담당자가 맡는다.
- 실제 브라우저는 기존 테스트가 생성하는 새 Chromium 컨텍스트와 임시 프로젝트만 사용한다. 마이크·전사·브라우저 Clipboard API·새 네이티브 복사 API는 대역이다. 테스트에서 모든 `POST /clipboard`는 route로 차단해 실제 OS 클립보드로 전달하지 않는다. 외부 네트워크·유료 API·사용자 서버/프로젝트/설정은 사용하지 않는다.
- 기존 음성 타임라인 검증의 실제 start/stop 기준·오디오만 전송·선택 위치·원문 보존 조건과 파일/탭/패널 소비자 계약을 유지한다.

## 검증 계획

| ID | 입력/상태 | 기대 결과 |
| --- | --- | --- |
| CB-B01 | 전사 완료 후 browser write가 거절, 로컬 대체 성공 | 전체 발화·선택 타임라인을 같은 origin의 인증된 POST 한 번으로 전달하고 완료 알림, 수동 대화상자 없음 |
| CB-B02 | browser 거절과 로컬 대체 사용 불가 | 전체 지시문 보존, 원인 안내에 원문 서버 오류 노출 없음, 직접 클릭 재복사 성공 시 닫힘, 재전사 없음 |
| CB-B03 | browser write promise가 대기하는 동안 프로젝트 연결 무효화 | 늦은 reject 뒤 로컬 대체 요청·수동 창·오래된 지시문이 살아나지 않음 |
| CB-B03b | 네이티브 API 응답이 대기하는 동안 프로젝트 연결 무효화 | 요청이 취소되거나 늦은 성공을 무시하고 복사 알림·수동 창·오래된 지시문을 되살리지 않음 |
| CB-B04 | 파일 상대/절대/이름 직접 복사, 거절 후 양쪽 실패 | 성공은 브라우저에서 완료, 거절 시 같은 로컬 fallback 계약과 원문 보존·원본 파일 포커스 복귀 유지 |
| CB-B05 | 관련 기존 음성/파일/패널/저장 회귀 | 기존 사용자 결과 유지; desktop/mobile 각 여정 확인 |

새 반례는 이전 0.6.0 고정 바이너리에서 먼저 실행해 browser-only 구현의 실패를 보존한다. 구현 동결 바이너리를 받은 뒤 같은 기대 조건을 실행한다. 테스트 실행·수정 결과는 아래에 추가한다.

## 실행 기록

- 테스트 두 파일의 `node --check`와 담당 파일 `git diff --check`는 exit0이다. 최초 RED에는 이전 0.6.0 고정 바이너리 `target/architalk-audio-sync-final`, SHA256 `7335cf649050b9e230a8382a00b643af5132f8bcbf23943e233a45d9e13b8fc5`를 사용했다.
- 실행 `ARCHITALK_BROWSER_BINARY=target/architalk-audio-sync-final ARCHITALK_BROWSER_RUN=clipboard-native-red-20261118 npx playwright test tests/browser/document-voice.spec.mjs --project=desktop --grep 'clipboard denial uses one authenticated local copy'` 결과 **0 pass / 1 fail**이다. 브라우저 거절 후 복사 완료 알림이15초 안에 나타나지 않고 기존 수동 창만 표시되는 browser-only 제품 결함을 재현했다. 실패 PNG/문맥은 ignored `target/browser-artifacts/clipboard-native-red-20261118/`에 보존한다.
- 다음 실행은 root가 새 구현을 동결한 바이너리에서 음성16사례와 파일/탭/패널/저장4사례를 확인한다. 수동 복사 창의 두 버튼이 desktop/mobile 뷰포트에 들어오는지 검사하고 mobile PNG도 읽는다. 성공 기준을 줄이거나 실패를 재시도로 감추지 않는다.
- root 동결 바이너리 `target/architalk-clipboard-fix`는0.6.1이며 실제 SHA256은 `4b15907f1616bc543ac3f40dfe08b826927f42f6d01638edf85a5026986e12b6`로 확인했다. `ARCHITALK_BROWSER_BINARY=target/architalk-clipboard-fix ARCHITALK_BROWSER_RUN=clipboard-fix-20261118 npx playwright test tests/browser/document-voice.spec.mjs tests/browser/viewer-workspace.spec.mjs` 결과 **20 pass / 0 fail**,25.9초다. 첫 실행에서 모두 통과했으며 재시도는0이다.
- 거절된 browser 호출의 전체 텍스트와 네이티브 요청 body의 텍스트가 일치하고, 같은 origin/정확한 POST 경로/인증 헤더/JSON 필드 하나/요청1회를 확인했다. 실제 발화0.2초·1.5초와 선택1.5초가 보존되고 오디오는 전사 요청에만 포함됐다. 두 복사 실패는 고정 사유와 전체 텍스트를 유지하고, 원시 browser/server 오류를 노출하지 않으며, 직접 클릭 재시도는 새 전사/로컬 요청 없이 성공했다.
- browser promise 대기와 native 응답 대기 중 각각 workspace410을 주입했다. 늦은 결과는 새 네이티브 요청/완료 알림/수동 창/이전 지시문을 되살리지 않았다. 기존 키 사전 검사·설정 유지/삭제·start/stop 시각·audio-only 전송·마이크 해제·파일/탭/패널·10종 저장 소비자도 통과했다.
- mobile 수동 복사 PNG를 직접 읽었다.390×844 화면에서 긴 실패 사유, 텍스트 입력 영역, 하단 `닫기`/`복사` 버튼이 한 화면에 들어오고 겹침·가로 넘침은 없었다. 두 버튼의 뷰포트 포함을 자동 검사했다. scoped CSS 수정은 필요하지 않았다. 산출물은 ignored `target/browser-artifacts/clipboard-fix-20261118/`에 보존한다.
- 남은 한계: 이 브라우저 실행은 새 인증 API에 대한 프런트엔드 연동 계약을 검증한 것이다. OS 명령 성공·실패·권한·취소·보존 경계는 서버 담당의 fake-command 검증으로 별도 확인한다. 실제 사용자 클립보드·마이크·유료 전사 호출은 실행하지 않았다.
