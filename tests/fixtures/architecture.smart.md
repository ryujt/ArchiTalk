---
title: 알림 서비스 아키텍처
locale: ko
style: smartmd
---

# 알림 서비스 아키텍처

:::info title="설계 범위와 근거"
이 문서는 구조 검사용 합성 예제다. 자료 목록과 상세를 보여 주는 UI 및 읽기 전용 저장소를 제안한다. 실제 도메인 정책·부하·배포 환경은 미확정이며 담당자 확인과 사용 시나리오 검증이 필요하다.
:::

## 목적과 요구사항

{table caption="요구사항과 확인 상태" columns=[{title="요구사항", align=left, width=60%}, {title="근거와 검증", align=left, width=40%}]}

| 요구사항 | 근거와 검증 |
| --- | --- |
| 목록에서 자료 상세를 연다 | 합성 예제의 제안, 목록·상세 시나리오로 확인 |
| 조회 실패 시 목록에서 다시 선택한다 | 합성 예제의 제안, 실패 주입으로 확인 |

## 구조와 계약

Reader가 열기 과업을 조율하고 Repository가 읽기를 담당한다. 상세 조회 입력은 자료 식별자이며 결과는 자료 또는 조회 실패다. 저장소·배포·성능 목표는 unknown이다. 인메모리 저장소로 시나리오를 확인한 뒤 실제 환경을 결정한다. 제외 범위는 자료 편집과 인증이다. 제안 UI와 논리 읽기 계약이며 실제 endpoint를 가정하지 않는다.

## Navigation

### 목록에서 상세 열기와 복귀

```navigation
ResourceList --> (open_resource) : 자료 선택
(open_resource) --> ResourceDetail : 조회 성공
(open_resource) --> ResourceList(LoadError) : 조회 실패
ResourceList(LoadError) --> ResourceList : 다시 선택
ResourceDetail --> ResourceList : 목록으로 복귀
```

목록에서 자료를 선택해 상세를 연다. 조회 실패 시 목록의 오류 장면에 남고 다시 선택할 수 있다. 상세에서 목록으로 복귀한다. 이 문서가 제안하는 과업이며 실제 구현 관찰을 주장하지 않는다.

### 화면 확인 링크

| 페이지·장면 | 화면 HTML | Layout |
| --- | --- | --- |
| 자료 목록·조회 오류 | 확인 불가 — 화면 HTML 미생성 | [자료 목록 배치](#resource-list) |
| 자료 상세·기본 | 확인 불가 — 화면 HTML 미생성 | [자료 상세 배치](#resource-detail) |

## Jobflow

```jobflow
orchestrator: Reader
Object: Reader, Repository
Reader.Open --> Repository.Load
Repository.Load.found --> Reader.ShowDetail
Repository.Load.failed --> Reader.ShowError
```

- 자료 열기를 시작하면 Reader가 Repository에 읽기를 요청한다.
- 자료를 찾으면 Reader가 상세를 표시한다.
- 조회가 실패하면 Reader가 목록에 오류를 표시한다.

## State

상태 소유자는 Reader의 단일 조회 과업이다. 목록 준비 시 초기화하며 자료 선택, 조회 성공·실패 및 재선택을 입력으로 받는다. Running 중 중복 열기 입력은 거부하고 실패가 자료 존재를 뜻하지 않는다는 불변조건을 유지한다. 화면 종료와 동작 취소 정책은 검증할 미확정 항목이다.

```state
<s> --> (Ready)
(Ready) --> (Running) : 자료 선택
(Running) --> (Showing) : 조회 성공
(Running) --> (Failed) : 조회 실패
(Failed) --> (Ready) : 다시 선택
(Showing) --> (Ready) : 목록 복귀
```

## Screen Layout

### Resource List

```layout
ListScreen V ListHeader, ListMain, ListFeedback
ListMain V ResourceRows, RetryAction
```

ListFeedback에서 조회 중·비어 있음·조회 실패를 설명하고 RetryAction으로 재선택한다. 논리적 제안 배치이며 키보드 순서는 제목, 자료, 재시도 순서다. 작은 화면에서는 세로 배치를 유지한다.

### Resource Detail

```layout
DetailScreen V DetailHeader, DetailBody, ReturnAction
```

자료 제목과 본문 뒤에 목록 복귀 동작을 둔다. 실제 화면 HTML은 미생성이며 두 배치 모두 구현 전에 사용자 검증이 필요하다.

## 검증과 대안

단일 목록 UI와 별도 상세 UI를 비교해 뒤로 가기와 조회 실패 복구를 확인한다. 읽기 실패를 주입해 State와 Navigation의 결과가 일치하는지 검사한다. 실제 인터페이스와 성능·운영 담당자는 미확정이며 다음 설계 검토에서 정한다. 사용자 승인이나 배포 완료를 주장하지 않는다.
