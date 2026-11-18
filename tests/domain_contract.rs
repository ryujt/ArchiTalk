use architalk::{domain, model::*};
use serde_json::{Value, json};
fn candidate(value: Value) -> domain::ConversationCandidate {
    serde_json::from_value(value).unwrap()
}
fn apply(s: &mut Session, value: Value) {
    domain::apply_conversation(s, candidate(value)).unwrap();
}
fn source(s: &Session) -> String {
    s.messages
        .iter()
        .rev()
        .find(|m| m.role == "user")
        .unwrap()
        .id
        .clone()
}
fn initial() -> Session {
    let mut s = domain::new_session(
        "보관 정책".into(),
        "개인정보는 30일 보관합니다.".into(),
        vec![],
    );
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"보관 정책을 제안으로 정리했습니다.","claims":[{"text":"개인정보 30일 보관","kind":"interpretation","evidence_refs":[reference],"source_quote":"개인정보는 30일 보관합니다."}],"requirements":[{"text":"개인정보 보관 기간은 30일이다.","acceptance":"30일이 지나면 보관하지 않는다.","evidence_refs":[reference],"source_quote":"개인정보는 30일 보관합니다."}],"decisions":[{"title":"보관 정책","alternatives":["30일 보관","즉시 삭제"],"rationale":"사용자가 30일을 제시했다.","consequences":"30일 만료 처리가 필요하다.","evidence_refs":[reference],"source_quote":"개인정보는 30일 보관합니다."}]}),
    );
    s
}
#[test]
fn correction_supersedes_versions_preserves_sources_and_marks_dependents_stale() {
    let mut s = initial();
    let old_req = s.requirements[0].id.clone();
    let old_claim = s.claims[0].id.clone();
    let old_dec = s.decisions[0].id.clone();
    s.decisions[0].requirement_refs = vec![old_req.clone()];
    let old_message = s.messages[0].clone();
    let old_revision = s.design_revision;
    domain::add_answer(&mut s, "아니요, 개인정보는 7일만 보관합니다.".into());
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"최신 정정에 따른 변경 제안을 기록했습니다.","claims":[{"supersedes":old_claim,"text":"개인정보 7일 보관","kind":"interpretation","evidence_refs":[reference],"source_quote":"개인정보는 7일만 보관합니다."}],"requirements":[{"supersedes":old_req,"text":"개인정보 보관 기간은 7일이다.","acceptance":"7일이 지나면 보관하지 않는다.","evidence_refs":[reference],"source_quote":"개인정보는 7일만 보관합니다."}],"decisions":[{"supersedes":old_dec,"title":"보관 정책","alternatives":["7일 보관","즉시 삭제"],"rationale":"최신 사용자 정정에 근거한 제안이다.","consequences":"7일 만료 처리가 필요하다.","requirement_refs":[old_req],"evidence_refs":[reference],"source_quote":"개인정보는 7일만 보관합니다."}]}),
    );
    assert_eq!(s.requirements[0].status, "superseded");
    assert_eq!(s.claims[0].status, "superseded");
    assert_eq!(s.decisions[0].status, "superseded");
    assert_eq!(s.requirements.len(), 2);
    assert_eq!(s.claims.len(), 2);
    assert_eq!(s.decisions.len(), 2);
    assert_eq!(
        s.requirements[1].supersedes.as_deref(),
        Some(old_req.as_str())
    );
    assert_eq!(
        s.decisions[1].requirement_refs,
        vec![s.requirements[1].id.clone()]
    );
    assert_eq!(s.messages[0].content, old_message.content);
    assert_eq!(
        s.requirements[0].acceptance,
        "30일이 지나면 보관하지 않는다."
    );
    assert_eq!(s.requirements[1].status, "proposed");
    assert_eq!(s.claims[1].status, "proposed");
    assert!(s.design_revision > old_revision);
    let prompt = domain::prompt(&s, "design", "ko").unwrap();
    assert!(prompt.contains("active_requirements"));
    assert!(prompt.contains("superseded_requirements"));
    let appendix = domain::trace_appendix(&s);
    assert!(appendix.contains("superseded"));
    let current = appendix.split("### 대체된 이력").next().unwrap();
    assert!(current.contains("개인정보 보관 기간은 7일이다."));
    assert!(!current.contains("개인정보 보관 기간은 30일이다."));
    assert!(current.contains("현재 해석과 결정 제안"));
}
#[test]
fn same_text_changed_acceptance_and_evidence_creates_an_audited_version() {
    let mut s = initial();
    let req = s.requirements[0].clone();
    domain::add_answer(
        &mut s,
        "보관 정책은 기존과 같지만, 7일 만료 후 삭제 여부를 검증하겠습니다.".into(),
    );
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"검증 기준 변경을 제안으로 기록했습니다.","requirements":[{"id":req.id,"text":req.text,"acceptance":"7일 만료 후 실제 삭제를 확인한다.","evidence_refs":[reference],"source_quote":"7일 만료 후 삭제 여부를 검증하겠습니다."}]}),
    );
    assert_eq!(s.requirements.len(), 2);
    assert_eq!(s.requirements[0].status, "superseded");
    assert_eq!(
        s.requirements[1].acceptance,
        "7일 만료 후 실제 삭제를 확인한다."
    );
    assert_eq!(s.requirements[1].evidence_refs, vec![reference]);
    assert_ne!(s.requirements[0].id, s.requirements[1].id);
}
#[test]
fn anonymous_same_text_amendment_is_validated_instead_of_silently_dropped() {
    let mut s = initial();
    let req = s.requirements[0].clone();
    domain::add_answer(&mut s, "같은 보관 요구에 만료 검증도 추가합니다.".into());
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"추가 수락 조건을 변경 제안으로 기록했습니다.","requirements":[{"text":req.text,"acceptance":"만료 시점의 삭제를 자동 검증한다.","evidence_refs":[reference],"source_quote":"만료 검증도 추가합니다."}]}),
    );
    assert_eq!(s.requirements.len(), 2);
    assert_eq!(s.requirements[0].status, "superseded");
    assert_eq!(
        s.requirements[1].supersedes.as_deref(),
        Some(req.id.as_str())
    );
}
#[test]
fn stale_or_unknown_amendment_sources_are_rejected_atomically() {
    for unknown in [false, true] {
        let mut s = initial();
        let req = s.requirements[0].id.clone();
        let old_reference = s.messages[0].id.clone();
        domain::add_answer(
            &mut s,
            if unknown {
                "아직 보관 기간을 모르겠어요."
            } else {
                "아니요, 개인정보는 7일만 보관합니다."
            }
            .into(),
        );
        let before = serde_json::to_value(&s).unwrap();
        let reference = if unknown { source(&s) } else { old_reference };
        let quote = if unknown {
            "아직 보관 기간을 모르겠어요."
        } else {
            "개인정보는 30일 보관합니다."
        };
        let result = domain::apply_conversation(
            &mut s,
            candidate(
                json!({"reply":"변경 제안입니다.","requirements":[{"supersedes":req,"text":"보관 기간을 1년으로 변경한다.","acceptance":"1년 보관한다.","evidence_refs":[reference],"source_quote":quote}]}),
            ),
        );
        assert!(result.is_err());
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
}
#[test]
fn risk_fallback_is_considered_even_when_model_proposes_highest_scope_priority() {
    let mut s = domain::new_session(
        "결제 보조".into(),
        "작은 결제 도구를 만듭니다.".into(),
        vec![],
    );
    apply(
        &mut s,
        json!({"reply":"결제 도구의 목적을 정리했습니다.","questions":[{"target_ref":"scope","text":"원하는 색상은 무엇인가요?","reason":"화면 범위를 정합니다.","priority_class":1}]}),
    );
    let q = domain::select_question(&s).unwrap();
    assert!(s.coverage.iter().any(|c| c.id == q.target_ref && c.blocker));
    assert_eq!(q.priority_class, 2);
}
#[test]
fn known_conflict_precedes_risk_and_model_priority_is_not_authoritative() {
    let mut s = domain::new_session("결제".into(), "결제 도구를 만듭니다.".into(), vec![]);
    let scope = s.coverage.iter_mut().find(|c| c.id == "scope").unwrap();
    scope.assessment_status = "conflict".into();
    apply(
        &mut s,
        json!({"reply":"상충한 범위를 먼저 확인합니다.","questions":[{"target_ref":"payment_recovery","text":"누가 복구하나요?","reason":"금전 실패를 복구합니다.","priority_class":1}]}),
    );
    let q = domain::select_question(&s).unwrap();
    assert_eq!(q.target_ref, "scope");
    assert_eq!(q.priority_class, 1);
}
#[test]
fn summary_cannot_smuggle_extra_questions_and_question_text_is_single() {
    for (reply, text) in [
        (
            "어떤 기능인가요? 사용자는 누구인가요?",
            "범위는 무엇인가요?",
        ),
        (
            "현재 목적을 이해했습니다.",
            "기능은 무엇인가요? 사용자는 누구인가요?",
        ),
    ] {
        let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
        let before = serde_json::to_value(&s).unwrap();
        let result = domain::apply_conversation(
            &mut s,
            candidate(
                json!({"reply":reply,"questions":[{"target_ref":"scope","text":text,"reason":"범위를 확인합니다."}]}),
            ),
        );
        assert!(result.is_err());
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
    let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    apply(
        &mut s,
        json!({"reply":"현재 목적을 이해했습니다.","questions":[{"target_ref":"scope","text":"범위는 무엇인가요?","reason":"첫 범위를 확인합니다."}]}),
    );
    assert_eq!(s.messages.last().unwrap().content.matches('?').count(), 1);
}

#[test]
fn candidate_collection_field_and_total_limits_reject_without_mutating_state() {
    let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    let reference = source(&s);
    let before = serde_json::to_value(&s).unwrap();
    let requirement = json!({"text":"도구를 만든다.","acceptance":"동작을 확인한다.","evidence_refs":[reference],"source_quote":"작은 도구입니다."});
    let oversized = [
        json!({"reply":"요약","requirements":vec![requirement.clone();41]}),
        json!({"reply":"요약","requirements":[{"text":"x".repeat(8193),"acceptance":"확인","evidence_refs":[reference],"source_quote":"작은 도구입니다."}]}),
        json!({"reply":"요약","decisions":vec![json!({"title":"선택","alternatives":["A","B"],"rationale":"이유","consequences":"결과","evidence_refs":[reference],"source_quote":"작은 도구입니다."});21]}),
        json!({"reply":"요약","questions":[{"target_ref":"scope","text":"어떤 범위인가요?","reason":"범위를 확인합니다.","options":["A","B","C","D","E"]}]}),
        json!({"reply":"요약","requirements":(0..40).map(|n|json!({"text":format!("요구{n}"),"acceptance":"x".repeat(8000),"evidence_refs":[reference],"source_quote":"작은 도구입니다."})).collect::<Vec<_>>()}),
    ];
    for value in oversized {
        assert!(domain::apply_conversation(&mut s, candidate(value)).is_err());
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
}
#[test]
fn unsourced_requirement_and_decision_never_become_confirmed_facts() {
    let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    let reference = source(&s);
    assert!(domain::apply_conversation(&mut s,candidate(json!({"reply":"요약","requirements":[{"text":"월 사용자는 2천명이다.","acceptance":"2천명 지원","evidence_refs":[reference]}]}))).is_err());
    assert!(domain::apply_conversation(&mut s,candidate(json!({"reply":"요약","decisions":[{"title":"선택","alternatives":["A","B"],"rationale":"이유","consequences":"결과","evidence_refs":[reference],"source_quote":"사용자는 2천명입니다."}]}))).is_err());
    assert!(s.requirements.is_empty());
    assert!(s.decisions.is_empty());
}

#[test]
fn sliced_unknown_quote_cannot_authorize_supersession() {
    let mut s = initial();
    let req = s.requirements[0].id.clone();
    domain::add_answer(&mut s, "보관 기간은 아직 모르겠어요.".into());
    let reference = source(&s);
    let before = serde_json::to_value(&s).unwrap();
    let result = domain::apply_conversation(
        &mut s,
        candidate(
            json!({"reply":"수정 제안입니다.","requirements":[{"supersedes":req,"text":"보관 기간은 1년이다.","acceptance":"1년 보관","evidence_refs":[reference],"source_quote":"보관 기간"}]}),
        ),
    );
    assert!(result.is_err());
    assert_eq!(serde_json::to_value(&s).unwrap(), before);
}
#[test]
fn historical_requirement_cannot_resurrect_without_an_explicit_current_target() {
    let mut s = initial();
    let old = s.requirements[0].clone();
    domain::add_answer(&mut s, "아니요, 개인정보는 7일만 보관합니다.".into());
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"정정을 제안으로 기록했습니다.","requirements":[{"supersedes":old.id,"text":"개인정보 보관 기간은 7일이다.","acceptance":"7일 만료 후 보관하지 않는다.","evidence_refs":[reference],"source_quote":"개인정보는 7일만 보관합니다."}]}),
    );
    let current = s.requirements[1].id.clone();
    domain::add_answer(&mut s, "추가 변경은 없습니다.".into());
    let before = serde_json::to_value(&s).unwrap();
    let historical = json!({"reply":"이전 내용을 재사용합니다.","requirements":[{"text":old.text,"acceptance":old.acceptance,"evidence_refs":old.evidence_refs,"source_quote":old.source_quote}]});
    assert!(domain::apply_conversation(&mut s, candidate(historical)).is_err());
    assert_eq!(serde_json::to_value(&s).unwrap(), before);
    domain::add_answer(&mut s, "다시 개인정보는 30일 보관합니다.".into());
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"명시적 재변경을 제안으로 기록했습니다.","requirements":[{"supersedes":current,"text":old.text,"acceptance":old.acceptance,"evidence_refs":[reference],"source_quote":"개인정보는 30일 보관합니다."}]}),
    );
    assert_eq!(
        s.requirements
            .iter()
            .filter(|r| r.status != "superseded")
            .count(),
        1
    );
    assert_eq!(
        s.requirements.last().unwrap().supersedes.as_deref(),
        Some(current.as_str())
    );
}
#[test]
fn historical_claim_and_decision_cannot_resurrect_anonymously() {
    let mut s = initial();
    let old_claim = s.claims[0].clone();
    let old_decision = s.decisions[0].clone();
    domain::add_answer(&mut s, "아니요, 개인정보는 7일만 보관합니다.".into());
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"변경을 제안으로 기록했습니다.","claims":[{"supersedes":old_claim.id,"text":"개인정보 7일 보관","evidence_refs":[reference],"source_quote":"개인정보는 7일만 보관합니다."}],"decisions":[{"supersedes":old_decision.id,"title":"7일 보관 정책","alternatives":["7일 보관","즉시 삭제"],"rationale":"사용자 정정에 근거한 제안","consequences":"7일 만료 필요","evidence_refs":[reference],"source_quote":"개인정보는 7일만 보관합니다."}]}),
    );
    let before = serde_json::to_value(&s).unwrap();
    for value in [
        json!({"reply":"과거 내용을 재사용합니다.","claims":[{"text":old_claim.text,"kind":old_claim.kind,"evidence_refs":old_claim.evidence_refs,"source_quote":old_claim.source_quote}]}),
        json!({"reply":"과거 내용을 재사용합니다.","decisions":[{"title":old_decision.title,"alternatives":old_decision.alternatives,"rationale":old_decision.rationale,"consequences":old_decision.consequences,"evidence_refs":old_decision.evidence_refs,"source_quote":old_decision.source_quote}]}),
    ] {
        assert!(domain::apply_conversation(&mut s, candidate(value)).is_err());
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
}
#[test]
fn readiness_requires_an_active_sourced_requirement_and_rechecks_exact_quotes() {
    let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    for c in &mut s.coverage {
        c.status = "covered".into();
    }
    let mut draft = Draft {
        id: id(),
        input_revision: s.design_revision,
        markdown: "# 설계".into(),
        documents: vec![],
        hash: "hash".into(),
        created_at: now(),
        content_status: "draft".into(),
        reviewed: true,
    };
    assert!(!domain::readiness(&s, &draft).is_empty());
    let reference = source(&s);
    apply(
        &mut s,
        json!({"reply":"검증 가능한 요구를 제안했습니다.","requirements":[{"text":"작은 도구를 제공한다.","acceptance":"도구의 핵심 동작이 실행된다.","evidence_refs":[reference],"source_quote":"작은 도구입니다."}]}),
    );
    draft.input_revision = s.design_revision;
    assert!(domain::readiness(&s, &draft).is_empty());
    s.requirements[0].source_quote = "없는 원문".into();
    assert!(!domain::readiness(&s, &draft).is_empty());
    s.requirements[0].status = "superseded".into();
    assert!(!domain::readiness(&s, &draft).is_empty());
}

#[test]
fn conversation_limit_contract_prompt_names_every_bound_in_utf8_bytes() {
    let s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    let prompt = domain::prompt(&s, "conversation", "ko").unwrap();
    let encoded = prompt
        .split("CONVERSATION_LIMITS_JSON:\n")
        .nth(1)
        .expect("conversation prompt must disclose the validator's numeric contract")
        .lines()
        .next()
        .unwrap();
    let limits: Value = serde_json::from_str(encoded).unwrap();
    assert_eq!(limits["units"], "UTF-8 bytes, not characters");
    assert_eq!(limits["no_nul"], true);
    for (path, maximum) in [
        ("claims", 40),
        ("requirements", 40),
        ("decisions", 20),
        ("coverage_updates", 30),
        ("questions", 20),
        ("questions[].options", 4),
        ("decisions[].alternatives", 5),
        ("evidence_refs", 40),
        ("requirement_refs", 40),
    ] {
        assert_eq!(limits["max_items"][path], maximum, "{path}");
    }
    for (path, maximum) in [
        ("reply", 8192),
        ("claims[].kind", 32),
        ("claims[].id", 200),
        ("requirements[].supersedes", 200),
        ("coverage_updates[].id", 200),
        ("questions[].target_ref", 200),
        ("questions[].text", 2000),
        ("questions[].reason", 8192),
        ("questions[].options[]", 1000),
        ("evidence_refs[]", 200),
        ("requirement_refs[]", 200),
        ("decisions[].alternatives[]", 8192),
    ] {
        assert_eq!(limits["max_bytes"][path], maximum, "{path}");
    }
    assert_eq!(limits["aggregate_text_max_bytes"], 262144);
    assert!(prompt.contains("normally return one candidate"));
}

#[test]
fn conversation_limit_contract_question_errors_identify_the_exact_bound_without_source_text() {
    let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    let before = serde_json::to_value(&s).unwrap();
    let mut errors = Vec::new();
    for question in [
        json!({"target_ref":"scope","text":"어떤 범위인가요?","reason":"범위를 확인합니다.","options":["A","B","C","D","E"]}),
        json!({"target_ref":"scope","text":"어떤 범위인가요?","reason":"범위를 확인합니다.","options":["가".repeat(334)]}),
        json!({"target_ref":"scope","text":"가".repeat(667),"reason":"범위를 확인합니다."}),
        json!({"target_ref":"scope","text":"어떤 범위인가요?","reason":"비밀\0본문"}),
    ] {
        let error = domain::apply_conversation(
            &mut s,
            candidate(json!({"reply":"현재 목적을 이해했습니다.","questions":[question]})),
        )
        .unwrap_err();
        errors.push(error.to_string());
        assert_eq!(
            serde_json::to_value(&s).unwrap(),
            before,
            "rejection must leave the complete session unchanged"
        );
    }
    assert_eq!(
        errors,
        vec![
            "questions[0].options: observed 5 items; limit 4 items",
            "questions[0].options[0]: observed 1002 UTF-8 bytes; limit 1000 UTF-8 bytes",
            "questions[0].text: observed 2001 UTF-8 bytes; limit 2000 UTF-8 bytes",
            "questions[0].reason: observed 1 NUL bytes; limit 0 NUL bytes",
        ]
    );
    assert!(
        errors
            .iter()
            .all(|error| !error.contains("비밀") && !error.contains("본문"))
    );
}

#[test]
fn conversation_limit_contract_exact_utf8_option_boundary_is_preserved() {
    let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    let options = vec![
        format!("{}a", "가".repeat(333)),
        "둘째".into(),
        "셋째".into(),
        "미정".into(),
    ];
    assert_eq!(options[0].len(), 1000);
    apply(
        &mut s,
        json!({"reply":"현재 목적을 이해했습니다.","questions":[{"target_ref":"scope","text":"어떤 범위인가요?","reason":"범위를 확인합니다.","options":options}]}),
    );
    assert!(
        s.questions
            .iter()
            .any(|question| question.target_ref == "scope" && question.options == options)
    );
}

#[test]
fn conversation_limit_contract_other_fields_keep_caps_and_report_safe_paths() {
    let mut s = domain::new_session("도구".into(), "작은 도구입니다.".into(), vec![]);
    let before = serde_json::to_value(&s).unwrap();
    let claim = json!({"text":"해석","kind":"interpretation","evidence_refs":["source"],"source_quote":"도구"});
    let requirement = json!({"text":"요구","acceptance":"동작 확인","evidence_refs":["source"],"source_quote":"도구"});
    let decision = json!({"title":"선택","alternatives":["A","B"],"rationale":"이유","consequences":"결과","evidence_refs":["source"],"source_quote":"도구"});
    let mut cases = vec![
        (json!({"reply":"x".repeat(8193)}), "reply"),
        (
            json!({"reply":"요약","claims":vec![claim.clone();41]}),
            "claims",
        ),
        (
            json!({"reply":"요약","decisions":vec![decision.clone();21]}),
            "decisions",
        ),
        (
            json!({"reply":"요약","requirements":vec![requirement.clone();41]}),
            "requirements",
        ),
        (
            json!({"reply":"요약","questions":vec![json!({"target_ref":"scope","text":"범위는 무엇인가요?","reason":"확인합니다."});21]}),
            "questions",
        ),
        (
            json!({"reply":"요약","coverage_updates":vec![json!({"id":"scope","reason":"확인","source_quote":"도구","evidence_refs":["source"]});31]}),
            "coverage_updates",
        ),
    ];
    let mut value = claim.clone();
    value["kind"] = json!("x".repeat(33));
    cases.push((json!({"reply":"요약","claims":[value]}), "claims[0].kind"));
    let mut value = claim.clone();
    value["id"] = json!("");
    cases.push((json!({"reply":"요약","claims":[value]}), "claims[0].id"));
    let mut value = claim;
    value["evidence_refs"] = json!(vec!["source"; 41]);
    cases.push((
        json!({"reply":"요약","claims":[value]}),
        "claims[0].evidence_refs",
    ));
    let mut value = requirement.clone();
    value["acceptance"] = json!("x".repeat(8193));
    cases.push((
        json!({"reply":"요약","requirements":[value]}),
        "requirements[0].acceptance",
    ));
    let mut value = decision.clone();
    value["alternatives"] = json!(vec!["alternative"; 6]);
    cases.push((
        json!({"reply":"요약","decisions":[value]}),
        "decisions[0].alternatives",
    ));
    let mut value = decision;
    value["requirement_refs"] = json!(["가".repeat(67)]);
    cases.push((
        json!({"reply":"요약","decisions":[value]}),
        "decisions[0].requirement_refs[0]",
    ));
    cases.push((json!({"reply":"요약","coverage_updates":[{"id":"scope","reason":"확인","source_quote":"x".repeat(8193),"evidence_refs":["source"]}]}),"coverage_updates[0].source_quote"));
    let mut value = requirement;
    value["acceptance"] = json!("x".repeat(8000));
    cases.push((
        json!({"reply":"요약","requirements":vec![value;40]}),
        "candidate.text_budget",
    ));
    for (value, path) in cases {
        let error = domain::apply_conversation(&mut s, candidate(value))
            .unwrap_err()
            .to_string();
        assert!(error.starts_with(&format!("{path}: observed ")), "{error}");
        assert!(
            error.contains("limit ") || error.contains("minimum "),
            "{error}"
        );
        assert_eq!(serde_json::to_value(&s).unwrap(), before);
    }
}
