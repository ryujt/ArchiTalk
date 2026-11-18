use architalk::{
    design_generation::{self, GeneratedDocument, GenerationKind},
    design_package::{self, DesignPackage, DocumentKind},
    domain,
    model::{DesignCheckpoint, DesignPart, Draft, id, now},
};
use serde_json::{Value, json};

fn package() -> DesignPackage {
    serde_json::from_str(include_str!("fixtures/design-package.json")).unwrap()
}

fn stages() -> Vec<GeneratedDocument> {
    let package = package();
    GenerationKind::ALL
        .into_iter()
        .map(|kind| GeneratedDocument {
            kind,
            markdown: if kind == GenerationKind::Architecture {
                package.markdown.clone()
            } else {
                package
                    .documents
                    .iter()
                    .find(|document| document.kind.filename() == kind.filename())
                    .unwrap()
                    .markdown
                    .replace(
                        "아키텍처-설계.md#method-r-module-design",
                        "아키텍처-설계.md",
                    )
                    .replace("네비게이션.md#screen-verification", "네비게이션.md")
            },
        })
        .collect()
}

fn response(document: &GeneratedDocument) -> String {
    serde_json::to_string(document).unwrap()
}

#[test]
fn ten_fixed_targets_validate_incrementally_and_assemble_without_loss() {
    assert_eq!(GenerationKind::ALL[0], GenerationKind::Drivers);
    assert_eq!(GenerationKind::ALL[9], GenerationKind::Architecture);
    assert_eq!(GenerationKind::ALL[7], GenerationKind::ScreenLayout);
    assert_eq!(GenerationKind::ALL[8], GenerationKind::Navigation);
    let mut completed = vec![];
    for document in stages() {
        let validated =
            design_generation::validate_response(&response(&document), document.kind, &completed)
                .unwrap();
        assert_eq!(validated, document);
        completed.push(validated);
        design_generation::validate_prefix(&completed).unwrap();
    }
    let assembled = design_generation::assemble(&completed).unwrap();
    assert_eq!(assembled.markdown, completed[9].markdown);
    assert_eq!(assembled.documents.len(), 9);
    for document in &assembled.documents {
        let staged = completed
            .iter()
            .find(|item| item.kind.filename() == document.kind.filename())
            .unwrap();
        assert_eq!(document.markdown, staged.markdown);
    }
    design_package::validate(assembled).unwrap();
    assert!(design_generation::assemble(&completed[..9]).is_err());
}

#[test]
fn response_schema_kind_order_and_size_are_strict() {
    let documents = stages();
    for text in [
        response(&documents[1]),
        json!({"kind":"drivers","markdown":documents[0].markdown,"filename":"chosen.md"})
            .to_string(),
        json!({"markdown":documents[0].markdown,"documents":[]}).to_string(),
        format!("```json\n{}\n```", response(&documents[0])),
        json!({"kind":"../../drivers","markdown":documents[0].markdown}).to_string(),
        json!({"kind":"drivers","markdown":"x".repeat(design_package::MAX_DOCUMENT_BYTES + 1)})
            .to_string(),
    ] {
        assert!(design_generation::validate_response(&text, GenerationKind::Drivers, &[]).is_err());
    }
    assert!(
        design_generation::validate_response(
            &response(&documents[1]),
            GenerationKind::QualityScenarios,
            &[]
        )
        .is_err()
    );
    assert!(design_generation::validate_prefix(&[documents[1].clone()]).is_err());
    assert!(
        design_generation::validate_prefix(&[documents[0].clone(), documents[0].clone()]).is_err()
    );
    let mut windows = documents[0].clone();
    windows.markdown = windows.markdown.replace('\n', "\r\n");
    assert_eq!(
        design_generation::validate_response(&response(&windows), windows.kind, &[]).unwrap(),
        documents[0]
    );
}

#[test]
fn pending_trace_obligations_must_be_defined_when_the_owner_arrives() {
    let documents = stages();
    design_generation::validate_prefix(&documents[..1]).unwrap();
    let mut missing = documents[1].clone();
    missing.markdown = missing.markdown.replace("QA-001", "QA-002");
    let error =
        design_generation::validate_response(&response(&missing), missing.kind, &documents[..1])
            .unwrap_err()
            .to_string();
    assert!(error.contains("QA-001"), "{error}");
    let mut backward = documents[1].clone();
    backward.markdown.push_str("\nDRV-999 is not defined.\n");
    assert!(
        design_generation::validate_response(&response(&backward), backward.kind, &documents[..1])
            .is_err()
    );
    assert_eq!(
        documents[0],
        stages()[0],
        "prior accepted source was mutated"
    );
}

#[test]
fn every_checkpoint_keeps_profiles_sections_trace_ownership_and_required_edges() {
    let documents = stages();
    for markdown in [
        documents[0]
            .markdown
            .replace("style: smartmd", "style: html"),
        documents[0]
            .markdown
            .replace("## Architectural drivers", "## Missing section"),
        documents[0].markdown.replace("### DRV-001", "### QA-001"),
        documents[0]
            .markdown
            .replace("ADR-001", "a future decision"),
        documents[0].markdown.replace("DRV-001", "DRV-0001"),
        format!(
            "{}\n### DRV-001 Duplicate\nQA-001 ADR-001\n",
            documents[0].markdown
        ),
        documents[0]
            .markdown
            .replace("## Business goals", "<pre>\n## Business goals"),
    ] {
        assert!(
            design_generation::validate_response(
                &json!({"kind":"drivers","markdown":markdown}).to_string(),
                GenerationKind::Drivers,
                &[]
            )
            .is_err()
        );
    }
    let mut jobflow = documents[5].clone();
    jobflow.markdown = jobflow.markdown.replace("```jobflow", "```mermaid");
    assert!(
        design_generation::validate_response(&response(&jobflow), jobflow.kind, &documents[..5])
            .is_err()
    );
}

#[test]
fn links_use_the_server_catalog_and_only_real_current_or_completed_anchors() {
    let documents = stages();
    for target in [
        "품질속성과-검증시나리오.md",
        "#architectural-drivers",
        "https://example.test/source",
    ] {
        let mut document = documents[0].clone();
        document
            .markdown
            .push_str(&format!("\n[reference]({target})\n"));
        design_generation::validate_response(&response(&document), document.kind, &[]).unwrap();
    }
    for target in [
        "품질속성과-검증시나리오.md#quality-scenarios",
        "#missing",
        "../outside.md",
        "invented.md",
        "javascript:alert(1)",
    ] {
        let mut document = documents[0].clone();
        document
            .markdown
            .push_str(&format!("\n[reference]({target})\n"));
        assert!(
            design_generation::validate_response(&response(&document), document.kind, &[]).is_err(),
            "accepted {target}"
        );
    }
    let mut quality = documents[1].clone();
    quality
        .markdown
        .push_str("\n[driver](요구사항과-아키텍처-드라이버.md#architectural-drivers)\n");
    design_generation::validate_response(&response(&quality), quality.kind, &documents[..1])
        .unwrap();
    quality
        .markdown
        .push_str("\n[missing](요구사항과-아키텍처-드라이버.md#missing)\n");
    assert!(
        design_generation::validate_response(&response(&quality), quality.kind, &documents[..1])
            .is_err()
    );
}

#[test]
fn restored_prefix_cannot_retroactively_legitimize_an_earlier_forward_anchor() {
    let mut documents = stages();
    documents[0]
        .markdown
        .push_str("\n[future](품질속성과-검증시나리오.md#quality-scenarios)\n");
    assert!(
        design_generation::validate_prefix(&documents[..2]).is_err(),
        "later checkpoint presence cannot bypass an earlier document's stage boundary"
    );
}

#[test]
fn aggregate_limit_and_final_package_rules_cannot_be_bypassed_by_checkpoints() {
    let mut documents = stages();
    for document in &mut documents[..6] {
        document.markdown.push_str(&"x".repeat(90_000));
    }
    assert!(design_generation::validate_prefix(&documents[..6]).is_err());
    let documents = stages();
    let mut main = documents[9].clone();
    let marker = "## Method-R module design\n";
    let start = main.markdown.find(marker).unwrap() + marker.len();
    let end = main.markdown[start..].find("\n## ").unwrap() + start;
    main.markdown
        .replace_range(start..end, "\nNo scoped orchestrator is supplied here.\n");
    assert!(
        design_generation::validate_response(&response(&main), main.kind, &documents[..9]).is_err(),
        "the final artifact must pass the original whole-package validator before commit"
    );
    assert_eq!(DocumentKind::ALL.len(), 9);
    design_package::validate(package()).unwrap();
}

#[test]
fn single_document_prompt_contains_complete_sources_prior_text_and_actual_manifest() {
    let documents = stages();
    let session = domain::new_session(
        "합성 문서".into(),
        "확인되지 않은 값은 unknown".into(),
        vec![],
    );
    let prompt = domain::prompt_design_document(
        &session,
        GenerationKind::QualityScenarios,
        &documents[..1],
        "ko",
    )
    .unwrap();
    assert!(prompt.contains(design_package::RULES));
    assert!(prompt.contains("UNTRUSTED DATA"));
    let target: Value =
        serde_json::Deserializer::from_str(prompt.split_once("TARGET_JSON:\n").unwrap().1)
            .into_iter::<Value>()
            .next()
            .unwrap()
            .unwrap();
    assert_eq!(target["kind"], "quality_scenarios");
    let context: Value =
        serde_json::from_str(prompt.split_once("CONTEXT_JSON:\n").unwrap().1).unwrap();
    assert_eq!(
        context["messages"][0]["content"],
        session.messages[0].content
    );
    assert_eq!(
        context["generation"]["completed_documents"][0]["markdown"],
        documents[0].markdown
    );
    let manifest = context["generation"]["manifest"].to_string();
    assert!(manifest.contains("DRV-001"));
    assert!(manifest.contains("architectural-drivers"));
    assert!(manifest.contains("QA-001"));
    assert!(manifest.contains("ADR-001"));
    let main_prompt = domain::prompt_design_document(
        &session,
        GenerationKind::Architecture,
        &documents[..9],
        "ko",
    )
    .unwrap();
    let main_context: Value =
        serde_json::from_str(main_prompt.split_once("CONTEXT_JSON:\n").unwrap().1).unwrap();
    for (index, document) in documents[..9].iter().enumerate() {
        assert_eq!(
            main_context["generation"]["completed_documents"][index]["markdown"],
            document.markdown
        );
    }
    assert!(
        domain::prompt_design_document(&session, GenerationKind::Drivers, &documents[..1], "ko")
            .is_err()
    );
}

#[test]
fn context_hash_tracks_prompt_sources_and_language_but_not_transport_or_drafts() {
    let mut session = domain::new_session("source".into(), "unknown".into(), vec![]);
    let before = domain::design_context_hash(&session, "ko").unwrap();
    session.revision += 7;
    session.active_run_id = Some("active-run".into());
    session.content_status = "approved".into();
    let part = stages().remove(0);
    session.design_generations.push(DesignCheckpoint {
        id: id(),
        input_revision: session.design_revision,
        input_hash: before.clone(),
        contract_hash: design_generation::contract_hash(),
        language: "ko".into(),
        completed: vec![DesignPart {
            content_hash: architalk::model::hash(&part.markdown),
            document: part,
            source_run_id: "earlier-run".into(),
            effective_settings: json!({"timeout_seconds":300,"model":"earlier-model"}),
            resolved_model: Some("earlier-model".into()),
            cli_version: "fixture".into(),
            created_at: now(),
        }],
        created_at: now(),
        updated_at: now(),
        draft_id: None,
    });
    let package = package();
    session.drafts.push(Draft {
        id: id(),
        input_revision: session.design_revision,
        hash: design_package::package_hash(&package.markdown, &package.documents),
        markdown: package.markdown,
        documents: package.documents,
        created_at: now(),
        content_status: "draft".into(),
        reviewed: false,
    });
    assert_eq!(before, domain::design_context_hash(&session, "ko").unwrap());
    session.design_generations[0].completed[0].effective_settings =
        json!({"timeout_seconds":1800,"model":"different-model","effort":"different"});
    session.design_generations[0].updated_at = "changed transport timestamp".into();
    assert_eq!(before, domain::design_context_hash(&session, "ko").unwrap());
    assert_ne!(before, domain::design_context_hash(&session, "en").unwrap());
    session.messages[0].content.push_str(" new evidence");
    assert_ne!(before, domain::design_context_hash(&session, "ko").unwrap());
    let changed = domain::design_context_hash(&session, "ko").unwrap();
    session.design_revision += 1;
    assert_ne!(
        changed,
        domain::design_context_hash(&session, "ko").unwrap()
    );
    let contract = design_generation::contract_hash();
    assert_eq!(contract.len(), 64);
    assert!(contract.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(contract, design_generation::contract_hash());
}

#[test]
fn combined_context_limit_fails_without_truncation_or_source_mutation() {
    let documents = stages();
    let mut session = domain::new_session("large".into(), "unknown".into(), vec![]);
    session.messages[0].content = "x".repeat(899_000);
    let original = session.messages[0].content.clone();
    let error = domain::prompt_design_document(
        &session,
        GenerationKind::QualityScenarios,
        &documents[..1],
        "ko",
    )
    .unwrap_err()
    .to_string();
    assert!(
        error.contains("context") && error.contains("900000"),
        "{error}"
    );
    assert_eq!(session.messages[0].content, original);
    assert_eq!(documents[0], stages()[0]);
}
