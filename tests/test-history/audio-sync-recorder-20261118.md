# 음성·문서 통합 타임라인 recorder 검증

## 범위와 사전 반례

- 기준 `6abe702`(0.5.0). 담당 범위는 `web/document-recorder.js`, `tests/viewer-workspace.test.mjs`, 이 검증 기록이다. backend/App/browser/문서는 다른 담당자가 소유한다. mdview recorder는 읽기 전용으로 조사했다.
- 실제 전사 세그먼트의 초 단위 `start/end`만 밀리초 타임라인에 사용한다. `timing:none` 및 구형 `{text,model}`은 문서 사건과 시간 대응을 만들지 않는다. full source transcript는 result.text에 보존한다.
- 사전 테스트는 역순/동시/겹침 발화, 같은 시각 speech 우선·동일 종류 안정 순서, 파일별 source/제목/anchor/행/offset/선택 text, blank segment, 잘못된 시간 거부, untimed fallback와 단일 통합 지시문을 판정한다.
- fake recorder는 실제 `start` 사건을 명시한다. 늦은 start 전에는 starting이고 사건이 없어야 하며, start 사건이 shared zero이다. stop 호출 순간의 시간/사건을 고정하고 지연 stop/upload가 이를 늘리지 않아야 한다. clear 뒤 지연 권한/start/stop/upload는 결과나 요청을 만들지 않는다.
- 기존 key preflight, audio-only POST 1회, size/event/text 제한, reset와 track cleanup도 유지한다. 모든 API/clock/media는 fake이며 외부 서비스·실제 microphone·clipboard는 사용하지 않는다.

## 실행 기록

- 구현 전에 `node --test tests/viewer-workspace.test.mjs`를 실행한다. 이후 같은 표적과 구문/diff 검사를 실행하며 실패와 재검증을 구분해 기록한다.
- 첫 사전 실행은 **9 pass / 7 fail**, exit1, 약135ms였다. timeline export 부재, object 응답을 받지 못하는 기존 formatter, 실제 start 사건 전에 recording이 되는 결함을 확인했다.
- 구현 첫 실행은 **15 pass / 1 fail**, exit1, 약136ms였다. 실패 fixture의 원문 `전사문 원본`과 segment 문장들이 달랐는데 untimedSpeech가 비어야 한다는 이전 기대가 남아 있었다. root가 확정한 부분 전사 보존 계약에 맞춰 fixture의 full transcript를 실제 segment 원문과 일치시켰다. 제품의 fallback을 약화하지 않았다.
- blank segment도 잘못된 시간이 있으면 먼저 거부하고, 유효한 blank만 제외한다. 누락/null/빈 segment 목록은 full transcript를 untimed로 남긴다. `timing:segment` 외의 응답은 segment처럼 보이는 필드가 있어도 시간 대응에 쓰지 않는다. 모든 시간은 유한·비음수·safe 정수 밀리초로 검사하며 segment 초는 `Math.round(seconds*1000)`으로 변환한다.
- lifecycle/제한 회귀를 보강한 같은 표적은 **17 pass**, exit0, 약132ms였다. 사건500개·선택 text2000자·제목/anchor240자 제한과 이전 epoch의 queued timer 차단도 확인했다.
- 독립 reviewer의 `a part` vs `apart` 사례를 root가 수용했다. whitespace를 전부 지우면 full source speech의 의미 있는 단어 경계를 잃을 수 있으므로 비교는 NFC + 연속 whitespace 한 칸 + trim, segment text는 trim 후 공백으로 연결한다. 이 반례를 먼저 실행한 표적은 **0 pass / 1 fail**, exit1, 약114ms였다.
- 비교 수정 뒤 `node --test tests/viewer-workspace.test.mjs`는 **18 pass**, exit0, 약136ms였다. `node --check web/document-recorder.js`, scoped `git diff --check`도 exit0이었다. API/media/clock는 모두 fake이며 실제 마이크·오디오 provider·clipboard는 호출하지 않았다.
- reviewer는 역순 segment 입력과 원문 순서가 일치해도 출력 timeline은 시각순 정렬되므로 원문이 clipboard에서 생략될 수 있음을 지적했고 root가 수용했다. 기존 역순 반례에 full raw transcript가 untimed 구역에도 남아야 한다는 판정을 먼저 넣은 표적은 **0 pass / 1 fail**, exit1, 약122ms였다. 비교 대상을 실제 정렬 완료한 speech timeline 문장으로 바꿨다.
- 최종 동일 표적은 **18 pass**, exit0, 약128ms였다. recorder 구문 검사와 scoped diff 검사는 exit0이었다. source/test/history를 최종 동결한다. browser 및 전체 소비자 결과는 root 담당이다.
