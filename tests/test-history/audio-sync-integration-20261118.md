# 발화·문서 조작 타임라인 통합 검증

## 사전 범위와 계약

- 기준 `6abe702`(0.5.0), 시작 시 clean main이다. 사용자 요청은 mdview처럼 음성 발화 시간과 문서 클릭/선택 시간을 맞추는 기능이다. backend·recorder 작성자와 별도 Critic을 분리했고 root는 UI 안내/실제 브라우저/문서/배포를 맡는다. 모델 override는 사용하지 않았다.
- reference mdview는 whisper-1에 verbose_json/segment를 요청하고 실제 start/end를 ms로 바꿔 조작과 stable sort한다. 시간 없는 모델의 전체 전사문은 untimedSpeech에 둔다. 원본 저장소는 읽기만 했다.
- 새 기본은 whisper-1, 기존 명시 모델은 보존한다. 응답은 기존 text/model에 segments(start/end 초)/timing(segment|none)을 추가한다. malformed-present timing은 고정 오류로 거절하고 누락/빈 시간 정보는 명시 untimed로 둔다.
- 브라우저는 MediaRecorder start 이벤트를 공통0으로 삼고 stop 시각과 조작을 동결한다. 실제 발화를 ms로 변환해 조작과 정렬하며 같은 시각의 발화가 먼저다. 완전 대응을 확인할 수 없는 전체 원문도 보존한다. 시간이나 의미 대응을 임의 추정하지 않는다.
- 오디오/키/전사/문서 조작의 기존 보존·전송 경계,20MiB/120초, 프로젝트 AI1800/3600초는 유지한다. 실제 사용자 서버·프로젝트 DB·공용 비밀 설정·마이크·클립보드·유료 API/모델은 검증에 사용하지 않는다.

## 검증 계획

| ID | 검증 대상 |
| --- | --- |
| AS-I01 | backend 실제 segment 파싱·모델별 요청·범위/크기/타입·기존 explicit 설정 호환. local fake upstream만 사용 |
| AS-I02 | 순수 merger 순서/tie/단위/겹침/원문/metadata/legacy untimed, 늦은 start·stop·upload/reset 수명. media/clock 대역 |
| AS-B01 | 실제 문서 선택→음성·조작 통합 클립보드, start 지연·같은 시각·upload 지연에도 elapsed 고정, audio-only POST, 시간 없는 모델 안내 |
| AS-B02 | 키 preflight/설정 유지·삭제/copy fallback/track cleanup 및 파일·패널·저장 소비자 회귀 |
| AS-R01 | README/API/제한/기존 모델 보존 설명, 빌드/설치/커밋·푸시 |

root의 browser는 기존 voice spec의 fake recorder가 실제 start 이벤트를 보내도록 보강하고, controlled clock/진짜 segment 응답 및 untimed 응답을 각각 주입한다. 시작 지연을 실제 녹음 시간으로 잘못 세거나 전사 네트워크 대기 시간을 elapsed에 더하는 경우를 검사한다. 결과·실패·수정은 아래에 남긴다.

## 실행 기록

- backend 표적은 transport9/9·HTTP7/7, recorder 표적은18/18 통과 후 소스 동결했다. 최초 RED와 수정은 각 담당 이력에 보존한다. Critic이 단어 경계 차이와 구간 정렬 순서 차이로 원문이 누락될 수 있는 사례를 찾아, 정렬된 발화와 NFC/연속 공백 정리 후 비교하고 불일치 원문을 보존하도록 수정했다. read-only 반례 재검증에 남은 blocker는 없었다.
- root `npm run test:web` 결과 **68 pass / 0 fail / 1 opt-in skip**,657ms. `cargo check --offline`, `cargo fmt --check`, `node --check web/app.js`, `node --check tests/browser/document-voice.spec.mjs`, `git diff --check` 모두 exit0이다. 버전은0.6.0, DB/schema/프로젝트 AI 제한은 변경하지 않는다.
- README/API에 기본 모델·기존 명시 설정 보존·실제 구간과 untimed 처리·시작/중지 기준·API 종료 일정을 반영했다. 실제 사용자 설정은 읽거나 덮어쓰지 않았다.
- 고정 바이너리 `target/architalk-audio-sync`는0.6.0, SHA256 `9099051b461b50c747d99a568d7004127e0a3498ee68404975b8f761c11ebdba`다. root voice 명령은 `ARCHITALK_BROWSER_BINARY=target/architalk-audio-sync ARCHITALK_BROWSER_RUN=audio-sync-voice-20261118 npx playwright test tests/browser/document-voice.spec.mjs`이며 결과 **10/10 pass**,9.8초다. 실제 start를4초 늦추고 stop 뒤 clock을93초 전진시킨 대역에서도 elapsed는2초로 고정되며 발화0.2s/1.5s와 선택1.5s가 정렬되고 같은 시각 발화가 먼저였다. Untimed 모델의 명시 안내·기존 key/copy/cleanup도 통과했다.
- `cargo test --locked`: **185 pass / 0 fail / 12 ignored**(중복 child probe summary 제외). ignored는 기존 실 CLI/모델·구 바이너리 opt-in이다. 다음으로 clippy all-targets, 관련 workspace/sidebar/file/save browser14사례를 같은 binary에서 확인하고 release 설치를 준비한다. UI·저장소 정책과 관계없는 전체 CSS 회귀50개를 재반복하지 않는다.
- 관련 consumer14/14(37.9초)와 all-target clippy가 통과했다. root가 실제 timed mobile PNG를 읽던 중, 문서 선택 뒤 녹음 중지 버튼의 mouseup도 같은 선택을 다시 기록하는 구체적 중복을 확인했다. source의 문서 밖 mouseup도 전역 selection을 수집하는 흐름이 원인이다. 선택 marker1회 보존 반례를 추가해 이전 binary에서 재현한 뒤, 실제 문서 영역의 mouseup만 선택으로 기록하도록 제한하고 음성 소비자만 재검증한다.
- 반례 최초 실행 `audio-sync-selection-red-20261118`: **0 pass / 1 fail**,2회 선택이1회 기대에 실패했다. 문서 영역 밖 mouseup을 제외하는 단일 UI listener 수정을 적용했다. 그 외 backend/recorder/UI source는 이전 동결 상태를 유지한다.
- 최종 고정 바이너리 `target/architalk-audio-sync-final` SHA256 `7335cf649050b9e230a8382a00b643af5132f8bcbf23943e233a45d9e13b8fc5`에서 음성10사례를 `audio-sync-voice-final-20261118`로 재검증한다. 같은 timeline 선택1회 조건과 clock/untimed/key/copy/cleanup 기준을 유지하며 테스트를 완화하지 않았다. Node68/1skip와 diff check도 다시 통과했다.
- 최종 음성 **10/10 pass**,9.7초. 같은 시각의 발화/선택 순서, 실제 시작 기준, upload 중2초 고정, 선택 marker1회, key/untimed/clipboard fallback/track cleanup 모두 통과했다. 관련 UI14는 앞선 같은0.6.0 source에서 검증했고 마지막 mouseup guard 변경의 영향을 받는 음성10만 재실행했다. final release build도 exit0,7.72초다.

## 배포 확인 계획

README/API와 기존 명시 모델 보존·기본whisper-1·모델별 시간 지원 안내를 반영했다. 실제 사용자 전사 설정/키를 읽거나 자동 변경하지 않는다. 설치 후 예전에 저장한 gpt-transcribe 모델은 UI의 음성 전사 설정에서 whisper-1로 바꿔야 발화 시각을 받을 수 있다. 전역0.6.0 설치·다른 폴더의 PATH/버전 확인 뒤 커밋·푸시한다. 실제 마이크/유료 API/실제 모델은 이번 검증에서 미실행이다.

- `./install.sh` exit0, 전역0.5.0을0.6.0으로 교체했다. `/tmp`의 새 login shell에서 `architalk --version`은 `architalk 0.6.0`, PATH는 기존 Cargo bin 실행 파일로 확인했다. 사용자 프로젝트 서버는 종료하지 않았다. 최종 diff check도 exit0이며 binary/PNG는 target에만 보관한다.
