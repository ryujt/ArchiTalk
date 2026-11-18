# 종류별 설계 문서 패키지의 저장·발행·복구 검증

## 사전 범위 — 2026-11-18

- 기준: `main@5a1a75e`에서 문서 렌더러 변경이 진행 중인 공유 작업 트리. 이번 Storage Developer의 소유는 `src/model.rs`, `src/files.rs`, `src/app.rs`, `src/storage.rs`, 파일·저장소 계약 및 패키지 workflow 테스트다. Domain/prompt/validator와 공용 DTO·고정 파일명은 다른 담당자가 소유한다.
- 목표: 주 문서와 9종 상세 문서를 하나의 불변 Draft로 저장하고 고정된 10파일로 발행한다. 모든 source 내용이 stale-preview 판정에 포함되고, legacy 단일 초안과 진행 중 export를 그대로 읽고 복구한다. 새 snapshot을 구버전 binary가 읽고 상세를 버리는 일을 SQLite schema version으로 차단한다.
- 선행 조건: root의 renderer 최종 브라우저 검증·렌더 커밋 및 명시적인 `쓰기시작` 통지. **현재는 계약 읽기와 준비 이력만 수행했으며 wired source 변경·빌드·테스트를 실행하지 않았다.** 이 보류는 공유 렌더 검증의 기준을 고정하기 위한 협업 순서다.
- 참고 이력: [implementation-storage](implementation-storage.md) ST-07~ST-09의 실제 macOS SQLite/dirfd/부분 발행·소유권 경계, [smartmd-documents](smartmd-documents-20261118.md)의 immutable 초안·형식 검증 및 자동 유료 재생성 금지, [document-rendering-integration](document-rendering-integration-20261118.md)의 직전 렌더 연결. 과거 통과를 새 패키지 경로의 증거로 사용하지 않는다.
- 스킬: 검증 우선 구현·종합 테스트의 작은 공개 계약부터 조립하고 실행별 이력을 남기는 규칙을 적용한다. 유료 모델, 외부 서비스, 사용자 프로젝트 데이터는 사용하지 않는다.

| 경계 | 변경·불변조건 | 필수 관찰 |
| --- | --- | --- |
| Draft | `documents` serde default; 새 hash는 정규화된 main+trace와 9종 전체, 빈 legacy는 기존 main hash | 과거 JSON 읽기·재저장 후 원문 보존; 상세만 변경하면 aggregate hash 변화 |
| App 생성 | 새 strict `validate_design_package`를 validate/complete 양쪽에 사용; 실패·취소·stale 시 새 초안 미추가 | 실제 격리 subprocess/SQLite 경로에서 전체 package 보존, 설계 자동 유료 재시도 없음 |
| Export preview | 서버 enum의 고정 filename만 사용; 상세에도 동일 검토 기준을 뒤에 추가하여 frontmatter 첫 바이트 유지 | 공개 preview의 10파일·각 hash·검토 상태·source hash 연결 |
| File publication | 기존 첫 주 문서 `document_hash`, manifest v1, dirfd authority, no-clobber·manifest-last 유지 | 10개 중 1개 충돌 시 나머지 미발행; 일부 존재 후 reconcile로 완성; 외부 bytes 보존 |
| App 복구 | 저장한 plan과 bundle identity로 기존 reconcile 사용 | 재시작 후 부분 패키지 완성 및 legacy 단일 export 복구 |
| SQLite 호환 | user_version 2로 forward guard; v1→v2 전 원본 backup, snapshot JSON 내용 변환 없음 | legacy snapshot·receipt·event 보존, backup은 v1, 다시 열 때 backup 중복 생성 없음, 미래 version 거절 |

## 계약 준비 기록

### PKS-P00 — 읽기·소유권·최소 함수 계약

- 시각: 2026-11-18 09:25 KST부터 준비. 실행 결과가 아니라 구현 전 읽기 기록이다.
- 확인: `ExportPlan.files`와 apply/reconcile는 이미 여러 파일을 지원한다. 단일 문서 생성만 작은 preview helper로 확장하고, `document_hash`의 첫 주 문서 의미와 manifest bytes 구조를 유지한다. `ExportEntry.source_document_hash`에는 Draft 전체 hash를 저장한다.
- DTO: `DesignDocument { kind, markdown }`, `DesignPackage { markdown, documents }`, `DocumentKind::ALL/filename`, `package_hash(markdown, documents)`를 사용한다. 모델은 filename을 선택하지 않는다.
- 필수 반례: 단일 legacy Draft 필드 부재, 상세만 바뀐 stale plan, 10개 중 외부 충돌, manifest 없는 부분 발행 후 재시작, 기존 v1 snapshot/진행 중 export, 구버전 손실 방지를 위한 schema marker.
- 상태: **구현·검증 미실행**. renderer 완료 신호 뒤 각 실제 명령의 시작·결과를 아래 새 ID로 기록한다. 공용 이력 색인은 root가 병합한다.

### PKS-S01 — 소유 파일 형식 및 첫 집중 계약 실행 준비

- 기준: renderer `d35a386` 통과 뒤 root의 쓰기 시작 통지. Domain 담당자의 새 validator·export metadata helper·DTO와 연결했다. schema2 전환 전 backup은 pinned/exclusive SQLite online backup을 사용하며 WAL의 committed page도 포함한다. root가 rusqlite의 기존 dependency에 `backup` feature를 추가했다.
- 예정 명령: 소유 Rust 파일의 `rustfmt --edition 2024`, `cargo test --test files_contract --test storage_contract --test design_package_workflow --test workflow -- --nocapture`. 시간은 실제 실행 도구 결과와 완료 항목에 기록한다.
- 필수 기준: 10파일 hash/고정 filename·검토 metadata·완료 경로, 마지막 파일 충돌 전 전체 사전검증, 부분 발행의 manifest-last 복구, revision/main 고정 상태의 상세 hash 변경 감지, legacy field 부재 및 schema1 진행 중 export 복구, v1 snapshot/receipt/event backup·이행, WAL backup.
- 증거 수준: 실제 macOS 파일·SQLite·공개 App/Store/ProjectFiles API와 격리 fixture subprocess. 대역 subprocess는 상태·프로토콜 연결만 검증하며 실제 모델 품질·할당량 증거가 아니다. 실제 모델 호출은 없다.
- 상태: 실행 직전. opt-in old-binary 거절 검증은 별도 실행 ID로 분리한다.

- PKS-S01 결과: **2026-11-18 09:33 KST**, rustfmt exit 0. 첫 집중 cargo 명령은 test helper가 `&App`로 공개 `&Arc<App>` receiver의 `command`를 호출하여 컴파일 오류 E0599, exit 101, 해당 묶음 실행 0건이었다. 제품 동작 실패로 해석하지 않는다. helper receiver를 실제 공개 계약에 맞게 수정했다.

### PKS-S02 — 집중 계약 재실행

- 명령·기준: PKS-S01의 동일 cargo 명령, helper receiver 수정 뒤 실행. 실제 결과 대기.

- PKS-S02 결과: **2026-11-18 09:33 KST**, package workflow **3 pass / 1 fail / 1 ignored**, cargo exit 101로 나머지 test binary 실행 전 중단. 실제 v1 재시작 경로에서 online backup의 새 파일명을 기존 dirfd VFS가 허용하지 않아 SQLite CANTOPEN을 관찰했다. 제품 연결 결함이며 backup을 일반 절대 경로로 우회하지 않는다. 기존 workspace DB/sidecar 외에 서버가 생성한 `workspace.pre-v1|2-<canonical UUID>.db`와 그 SQLite sidecar만 같은 whitelist에 추가했다.

### PKS-S03 — pinned backup filename 연결 수정 후 재실행

- 명령·기준: 동일 집중 cargo 명령. legacy 복구와 WAL-only backup이 실제 VFS를 거쳐 성공해야 한다. 결과 대기.

- PKS-S03 결과: **2026-11-18 09:34 KST**, exit 0. package workflow **4 pass / 1 opt-in ignored**, files contract **14 pass**, storage contract **12 pass**, controller workflow **7 pass**, 합계 **37 pass / 1 ignored / 0 fail**. macOS 실제 파일·SQLite·isolated subprocess를 사용했다. legacy 단일 export 재시작, package 부분 발행 재시작, WAL-only committed snapshot/receipt backup, 기존 symlink/hardlink/FIFO/root-swap 방어가 포함된다. ignored old-binary 사례를 통과로 합산하지 않는다.

### PKS-S04 — 실제 schema1 binary의 신규 DB 거절

- 명령: `ARCHITALK_LEGACY_BINARY=target/compat/architalk-schema1 cargo test --test design_package_workflow actual_schema1_binary_refuses_a_package_database_without_rewriting_its_bytes -- --ignored --nocapture`.
- 기준: root가 렌더 완료 뒤 보존한 실제 schema1 binary를 격리된 schema2 package DB에 실행한다. 5초 안에 `unsupported_schema`로 종료하고 원본 DB bytes가 동일해야 한다. 프로젝트 고정 경로는 테스트 코드에 넣지 않으며 환경변수로 executable을 받는다. 실제 모델 호출·브라우저 열기는 없다.
- 상태: 실행 직전.

- PKS-S04 결과: **2026-11-18 09:34:26 KST** 결과 관찰, exit 0, **1 pass / 0 ignored / 0 fail**. 실제 schema1 binary가 신규 package DB를 `unsupported_schema`로 거절했고 DB bytes가 보존됐다. 현재 코드의 version 비교를 복제한 대역이 아니다.

### PKS-S05 — 소유 변경의 형식·정적 검사

- 명령: 소유 Rust 파일에 `rustfmt --edition 2024`, `cargo clippy --lib --test files_contract --test storage_contract --test design_package_workflow --test workflow -- -D warnings`.
- 기준: 공유 소유 밖의 파일을 rustfmt로 변경하지 않고 이번 library/직접 계약 소비자가 경고 없이 검사된다. 실행 직전.

- PKS-S05 결과: **2026-11-18 09:34 KST**, 소유 파일 rustfmt exit 0, focused clippy exit 0·경고 0건. clippy는 동작 검증 건수에 합산하지 않는다.

## Storage 담당 판정·인계

- 이번 역할의 필수 집중 동작 **37건**과 opt-in 실제 old binary 거절 **1건**이 통과했다. Domain 담당자의 별도 새·legacy 형식 검증은 그 담당자 이력에서 확인한다. 공용 전체 회귀와 frontend package 선택·교차 문서 링크 및 최종 독립 Critic은 root의 통합 범위다.
- 새 DB는 schema2, 기존 v1은 online backup 뒤 version marker만 변경한다. legacy snapshot의 본문·hash와 manifest v1·첫 문서 hash 의미, reserve identity 및 no-clobber·manifest-last를 유지했다. 실제 모델 호출·배포·사용자 데이터 변경은 없다.
- 한계: 관찰 환경은 macOS 로컬 파일시스템이다. Linux 및 강제 전원 손실·디스크 고갈은 실행하지 않았다. 권한 실패·symlink/hardlink/FIFO/root swap, 수동 부분 발행 상태와 WAL-only commit이 이번 위험 범위의 실제 반례다. 이 증거를 실제 모델 문서 품질이나 모든 파일시스템의 원자성 검증으로 확대하지 않는다.
- 소스 변경은 root 통합·독립 검토를 위해 고정한다. 실패·수정·재실행의 근거는 PKS-S01→S02→S03, 실제 구버전 호환은 PKS-S04를 따른다.
