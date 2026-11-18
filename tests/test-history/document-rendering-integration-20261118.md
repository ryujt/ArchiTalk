# 설계 문서의 앱 내 실제 렌더링

- 기준: `5a1a75e`의 깨끗한 작업 트리. 사용자 스크린샷에서 검토 화면이 escaped `<pre>`로 SmartMD 원문을 표시하는 문제를 확인했다.
- 이전 근거: SmartMD 생성/저장과 mdview API 변환은 이전 이력에서 통과했지만 앱 내 시각 렌더는 구현하지 않았었다. [B03](chat-scroll-browser-20261118.md), [R04](smartmd-renderer-20261118.md)의 범위를 이번 검증과 구분한다.
- 소유: renderer Developer가 로컬 라이브러리/전용 DSL renderer/정화/CSS, root가 HTTP 내장 자산·CSP·앱 조립·공유 fixture·일반 회귀, 별도 Tester가 실제 브라우저와 적대적 입력, 별도 Critic이 코드/보안 경계를 확인한다. 상속 모델을 사용하고 실제 모델 호출은 하지 않는다.
- 기존 문서 원본과 사용자 프로젝트는 수정하지 않는다. 새 preview는 원문과 분리하고 기본으로 렌더링을 보여준다. 원문은 닫힌 details에서 볼 수 있다.

## RENDER-INT-01 — 조립 정적/계약 확인

- 실행 준비: `node --check web/app.js`, 기존 Node 21건, `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --locked`.
- 기준: 라이브러리는 바이너리에 내장하고 CDN 요청 없이 로드한다. script-src self, API 인증/Host/Origin 경계를 유지하며 다이어그램의 스타일만 inline 허용한다. 정화는 raw HTML의 script/제어 속성/폼·버튼/리소스/앱 DOM ID 및 delegated action을 차단해야 한다.
- 전체 workspace snapshot으로 재렌더할 때 같은 세션/초안/본문의 실제 document node를 재사용한다. 새 초안이나 화면 이동에서 이전 비동기 결과가 다른 문서를 덮지 않아야 한다.
- 브라우저 baseline 재현/시각 QA: [document-rendering-browser](document-rendering-browser-20261118.md). renderer 자체 검사: [document-renderer](document-renderer-20261118.md).

- 결과: 기존 Node 21건·Rust 80건(실제 모델 4건 ignored), format·all-target clippy 통과. 내장 notices 경로 보완 후 lib/bin clippy도 통과했다. 실제 모델 호출은 0건이다.
- 최종 브라우저 B08은 12건 모두 통과(31.7초). raw Markdown 대신 heading/table/4DSL/Mermaid, snapshot DOM 보존·새로고침, 잘못된 도표의 국소 오류, 모바일 도표 읽기와 원문 접기, 적대적 HTML/앱명령/네트워크/인증 누출을 확인했다.
- CSP 진단은 독립 최소 DOMParser(no Window)에서도 실행 없이 재현되는 Chromium 회귀였다. 정확한 pinned DOMPurify parser 진단만 별도 분류했고, 같은 적대적 문서는 테스트 페이지의 CSP를 제거한 조건에서도 실행·네트워크·제어 속성·인증 누출 0건이었다. 제품 script CSP는 완화하지 않았다. 실패·정정·공식 근거와 시각 QA는 B01~B08/D01~D04에 보존한다.
- 최종 코드에 대해 별도 Critic이 검토했고 발견된 layout 속성 삽입, SmartMD 안 코드 펜스, 진행 접근성, SVG 라벨·모바일 읽기 문제를 반영했다. vendor manifest와 배포 라이선스/출처를 바이너리에 포함했다.

## RENDER-INT-02 — 전역 설치

- 상태: 검증 통과한 renderer를 `./install.sh`로 전역 바이너리에 반영한다. 사용자 프로젝트 문서 원본/실행 중 서버·브라우저는 변경하지 않는다. 이어서 요청한 문서 패키지 기능은 별도 커밋·검증으로 진행한다.
- 결과: release 설치와 기존 전역 실행 파일 교체가 성공했다. 다음 문서 패키지의 DB forward-compatibility 검증을 위해 이 schema1 실행 파일만 Git 제외 `target/compat/architalk-schema1`에 보존했다. 사용자 DB나 실행 프로세스를 변경한 것이 아니다.
- 커밋 전 검사: 새로 stage한 upstream bundle/license의 원문 및 JS 문자열 내부 trailing whitespace가 일반 `git diff --check`에 잡혔다. 공급원 bytes와 manifest hash를 임의 정리하지 않고 `.gitattributes`에서 `web/vendor/**`의 whitespace 검사만 제외했다. 직접 작성한 소스의 검사는 유지한다.
