use architalk::{
    design_package::{self, DesignPackage, DocumentKind, MAIN_FILENAME},
    domain,
    model::*,
};
use serde_json::{Value, json};

const SAMPLE: &str = include_str!("fixtures/design-package.json");

fn sample() -> DesignPackage {
    serde_json::from_str(SAMPLE).unwrap()
}

fn module_link(package: &mut DesignPackage, target: &str) {
    let marker = "## Method-R module design\n";
    let start = package.markdown.find(marker).unwrap() + marker.len();
    let end = package.markdown[start..].find("\n## ").unwrap() + start;
    package.markdown.replace_range(start..end, &format!("\nReader는 작업자를 직접 연결하고 결과를 소유한다. [모듈 협력 계약]({target})의 실제 오케스트레이터 도표로 상세 협력을 설명한다.\n"));
}

#[test]
fn principal_module_can_link_to_the_actual_orchestrated_jobflow_heading() {
    let mut package = sample();
    module_link(&mut package, "잡플로우.md#open-resource");
    validate(&package).unwrap();
    let mut missing_macro = package;
    missing_macro.markdown = missing_macro.markdown.replace("```jobflow", "```mermaid");
    assert!(
        validate(&missing_macro).is_err(),
        "delegation never waives the principal macro diagram"
    );
}

#[test]
fn module_delegation_requires_a_real_link_and_orchestrator_in_its_exact_heading_scope() {
    for target in ["잡플로우.md", "잡플로우.md#missing", "상태도.md#read-task"] {
        let mut package = sample();
        module_link(&mut package, target);
        assert!(
            validate(&package).is_err(),
            "accepted unscoped or non-jobflow target {target}"
        );
    }
    let mut package = sample();
    module_link(&mut package, "잡플로우.md#boundary-only");
    let jobflow = package
        .documents
        .iter_mut()
        .find(|document| document.kind == DocumentKind::Jobflow)
        .unwrap();
    jobflow.markdown.push_str("\n### Boundary only\n\n외부 경계는 오케스트레이터 상세가 아니다.\n\n```jobflow\nscope: public cooperation\nObject: Client, Service\nClient.Request --> Service.Handle\n```\n\n### No diagram\n\n본문 계약만 있고 모듈 도표는 없다.\n");
    assert!(
        validate(&package).is_err(),
        "must not borrow the earlier Open resource orchestrator"
    );
    module_link(&mut package, "잡플로우.md#no-diagram");
    assert!(validate(&package).is_err());
    module_link(&mut package, "잡플로우.md#open-resource");
    package.markdown = package.markdown.replace(
        "[모듈 협력 계약](잡플로우.md#open-resource)",
        "`[모듈 협력 계약](잡플로우.md#open-resource)`",
    );
    assert!(
        validate(&package).is_err(),
        "inline code is not a module link"
    );
    module_link(&mut package, "잡플로우.md#open-resource");
    package.markdown = package.markdown.replace(
        "[모듈 협력 계약](잡플로우.md#open-resource)",
        "모듈 계약은 아래의 별도 절에서 설명한다.",
    );
    package
        .markdown
        .push_str("\n[다른 절의 링크](잡플로우.md#open-resource)\n");
    assert!(
        validate(&package).is_err(),
        "a link outside the module section cannot satisfy delegation"
    );
}

#[test]
fn module_link_duplicate_heading_suffix_selects_the_correct_diagram_scope() {
    let mut package = sample();
    let jobflow = package
        .documents
        .iter_mut()
        .find(|document| document.kind == DocumentKind::Jobflow)
        .unwrap();
    jobflow.markdown = jobflow.markdown.replace(
        "### Open resource",
        "### Task\n\n경계 설명만 있으며 도표는 다음 독립 과업에 있다.\n\n### Task",
    );
    module_link(&mut package, "잡플로우.md#task");
    assert!(validate(&package).is_err());
    module_link(&mut package, "잡플로우.md#task-1");
    validate(&package).unwrap();
}

#[test]
fn module_delegation_does_not_treat_images_escaped_links_or_bare_closers_as_links() {
    for fake in [
        "plain text](잡플로우.md#open-resource)",
        "![모듈 그림](잡플로우.md#open-resource)",
        "\\[모듈 협력 계약](잡플로우.md#open-resource)",
        "[모듈 협력 계약](잡플로우.md#open-resource invalid title)",
    ] {
        let mut package = sample();
        module_link(&mut package, "잡플로우.md#open-resource");
        package.markdown = package
            .markdown
            .replace("[모듈 협력 계약](잡플로우.md#open-resource)", fake);
        assert!(
            validate(&package).is_err(),
            "accepted a non-link delegation: {fake}"
        );
    }
}

#[test]
fn module_delegation_respects_an_indented_sibling_heading_scope() {
    let mut package = sample();
    module_link(&mut package, "잡플로우.md#target-scope");
    let jobflow = package
        .documents
        .iter_mut()
        .find(|document| document.kind == DocumentKind::Jobflow)
        .unwrap();
    jobflow.markdown.push_str("\n### Target scope\n\n여기는 도표 없는 계약 설명이다.\n\n  ### Other task\n\n다음 과업의 도표를 빌릴 수 없다.\n\n```jobflow\norchestrator: Owner\nObject: Caller, Owner\nCaller.Request --> Owner.Handle\n```\n");
    assert!(
        validate(&package).is_err(),
        "a valid indented sibling heading ends the target scope"
    );
}

#[test]
fn required_sections_cannot_be_hidden_inside_raw_html_blocks() {
    for tag in [
        "pre",
        "script",
        "style",
        "textarea",
        "div",
        "span",
        "custom-block",
    ] {
        let mut package = sample();
        let document = package
            .documents
            .iter_mut()
            .find(|document| document.kind == DocumentKind::Drivers)
            .unwrap();
        let (frontmatter, body) = document.markdown.split_once("\n---\n").unwrap();
        document.markdown = format!("{frontmatter}\n---\n<{tag}>\n{body}\n</{tag}>\n");
        assert!(
            validate(&package).is_err(),
            "accepted driver headings hidden by a raw {tag} block"
        );
    }
}

#[test]
fn raw_html_block_guard_preserves_inline_markup_comparisons_and_code_examples() {
    let mut package = sample();
    package.documents.iter_mut().find(|document| document.kind == DocumentKind::Drivers).unwrap().markdown.push_str(
        "\n문단의 <span>inline label</span>과 <b>강조</b>는 구조를 감추지 않는다. a < b 및 c > d 비교도 일반 문장이다.\n\n<span>한 줄의 inline 표시</span>\n\n`<pre>`와 `<script>`는 inline 코드다.\n\n```html\n<pre>\n## example only\n<script>example()</script>\n</pre>\n```\n\n    <div>indented code</div>\n",
    );
    validate(&package).unwrap();
}
fn validate(package: &DesignPackage) -> anyhow::Result<DesignPackage> {
    domain::validate_design_package(&serde_json::to_string(package).unwrap())
}
fn replace(package: &mut DesignPackage, kind: DocumentKind, before: &str, after: &str) {
    let document = package
        .documents
        .iter_mut()
        .find(|d| d.kind == kind)
        .unwrap();
    assert!(
        document.markdown.contains(before),
        "missing mutation input {before}"
    );
    document.markdown = document.markdown.replace(before, after);
}

#[test]
fn representative_package_has_ten_typed_documents_with_method_r_and_traceability() {
    let package = domain::validate_design_package(SAMPLE).unwrap();
    assert!(package.markdown.contains("# 알림 서비스 아키텍처"));
    assert_eq!(package.documents.len(), 9);
    for heading in ["macro", "system", "module", "detailed"] {
        assert!(
            package
                .markdown
                .contains(&format!("## Method-R {heading} design"))
        );
    }
    assert!(package.markdown.contains("scope: 자료조회 경계"));
    assert!(package.markdown.contains("orchestrator: 열기조율자"));
    for kind in DocumentKind::ALL {
        assert!(package.markdown.contains(kind.filename()));
    }
    // Supporting documents have their own analysis; the four DSLs are not
    // mechanically required in every companion document.
    let drivers = package
        .documents
        .iter()
        .find(|d| d.kind == DocumentKind::Drivers)
        .unwrap();
    assert!(!drivers.markdown.contains("```navigation"));
    let mut windows = sample();
    windows.markdown = windows.markdown.replace('\n', "\r\n");
    for document in &mut windows.documents {
        document.markdown = document.markdown.replace('\n', "\r\n");
    }
    let normalized = validate(&windows).unwrap();
    assert_eq!(normalized.markdown, package.markdown);
    assert_eq!(normalized.documents, package.documents);
}

#[test]
fn output_schema_rejects_missing_duplicate_unknown_and_model_owned_paths() {
    let mut missing = sample();
    missing.documents.pop();
    assert!(validate(&missing).is_err());
    let mut duplicate = sample();
    duplicate.documents[1].kind = duplicate.documents[0].kind;
    assert!(validate(&duplicate).is_err());
    for mutation in [
        "unknown_kind",
        "path",
        "filename",
        "extra_main",
        "missing_markdown",
    ] {
        let mut value: Value = serde_json::from_str(SAMPLE).unwrap();
        match mutation {
            "unknown_kind" => value["documents"][0]["kind"] = json!("../../outside"),
            "path" => value["documents"][0]["path"] = json!("../../outside.md"),
            "filename" => value["documents"][0]["filename"] = json!("custom.md"),
            "extra_main" => value["approved"] = json!(true),
            _ => {
                value.as_object_mut().unwrap().remove("markdown");
            }
        }
        assert!(
            domain::validate_design_package(&value.to_string()).is_err(),
            "accepted {mutation}"
        );
    }
    assert!(
        domain::validate_design_package(&json!({"markdown":sample().markdown}).to_string())
            .is_err()
    );
    assert!(domain::validate_design_package(&format!("```json\n{SAMPLE}\n```")).is_err());
}

#[test]
fn each_typed_body_needs_its_own_sections_and_common_frontmatter() {
    for kind in DocumentKind::ALL {
        let mut package = sample();
        let first = kind.required_sections()[0];
        replace(
            &mut package,
            kind,
            &format!("## {first}"),
            &format!("## Missing {first}"),
        );
        assert!(
            validate(&package).is_err(),
            "accepted missing section for {kind:?}"
        );
        let mut package = sample();
        replace(&mut package, kind, "style: smartmd", "style: plain");
        assert!(validate(&package).is_err(), "accepted plain {kind:?}");
    }
    let mut unsupported = sample();
    unsupported.documents[0]
        .markdown
        .push_str("\n:::unknown\n지원하지 않는 callout.\n:::\n");
    assert!(validate(&unsupported).is_err());
    let mut nonscalar = sample();
    replace(
        &mut nonscalar,
        DocumentKind::Drivers,
        "title: 요구사항과 아키텍처 드라이버",
        "title: [잘못된 배열]",
    );
    assert!(validate(&nonscalar).is_err());
    let mut package = sample();
    package.markdown = package
        .markdown
        .replace("## Method-R module design", "## Components");
    assert!(validate(&package).is_err());
    let mut package = sample();
    let document = package
        .documents
        .iter_mut()
        .find(|d| d.kind == DocumentKind::Drivers)
        .unwrap();
    let start = document.markdown.find("## Business goals\n").unwrap();
    let end = document.markdown[start + 1..].find("\n## ").unwrap() + start + 1;
    document
        .markdown
        .replace_range(start..end, "## Business goals\n\n");
    assert!(validate(&package).is_err());
}

#[test]
fn specialists_need_dedicated_dsl_and_cannot_hide_it_in_examples() {
    for (kind, language) in [
        (DocumentKind::Navigation, "navigation"),
        (DocumentKind::Jobflow, "jobflow"),
        (DocumentKind::State, "state"),
        (DocumentKind::ScreenLayout, "layout"),
    ] {
        let mut package = sample();
        replace(&mut package, kind, &format!("```{language}"), "```mermaid");
        assert!(validate(&package).is_err(), "accepted missing {language}");
        let mut package = sample();
        replace(
            &mut package,
            kind,
            &format!("```{language}"),
            &format!(" ```{language}"),
        );
        assert!(validate(&package).is_err(), "accepted indented {language}");
    }
    let mut package = sample();
    replace(
        &mut package,
        DocumentKind::Jobflow,
        "## Jobflow",
        "```text\n## Jobflow",
    );
    package
        .documents
        .iter_mut()
        .find(|d| d.kind == DocumentKind::Jobflow)
        .unwrap()
        .markdown
        .push_str("\n```\n");
    assert!(validate(&package).is_err());
    let mut package = sample();
    package.markdown = package.markdown.replace("```jobflow", "```mermaid");
    assert!(
        validate(&package).is_err(),
        "Method-R cannot be replaced by Mermaid"
    );
}

#[test]
fn confirmed_non_ui_exceptions_are_scoped_to_navigation_and_layout() {
    let waiver = "Applicability: not_applicable\nReason: 사용자가 확인한 배치 전용 도구로 화면을 제공하지 않는다.\nAlternative interface: CLI 입력 파일과 종료 코드·결과 파일로 과업을 확인한다.\n";
    let mut package = sample();
    for kind in [DocumentKind::Navigation, DocumentKind::ScreenLayout] {
        let doc = package
            .documents
            .iter_mut()
            .find(|d| d.kind == kind)
            .unwrap();
        doc.markdown = format!(
            "---\ntitle: 배치 인터페이스\nlocale: ko\nstyle: smartmd\n---\n\n# 배치 인터페이스\n\n## {}\n\n{waiver}\n{}",
            kind.required_sections()[0],
            "이 예외는 화면이 없다는 사용자 확인에만 적용하며 미확정 UI를 예외로 처리하지 않는다. 업무 과업과 실패 결과는 별도 Jobflow와 State 계약을 유지한다."
        );
    }
    // The navigation file no longer links layout anchors, but the remaining
    // package still has links to those inventory headings: preserve real ones.
    package
        .documents
        .iter_mut()
        .find(|d| d.kind == DocumentKind::ScreenLayout)
        .unwrap()
        .markdown
        .push_str("\n### Resource List\n\n화면 없음.\n\n### Resource Detail\n\n화면 없음.\n");
    validate(&package).unwrap();
    let mut unknown = package.clone();
    replace(
        &mut unknown,
        DocumentKind::Navigation,
        "Applicability: not_applicable",
        "Applicability: unknown",
    );
    assert!(validate(&unknown).is_err());
    let mut wrong = package;
    let doc = wrong
        .documents
        .iter_mut()
        .find(|d| d.kind == DocumentKind::State)
        .unwrap();
    doc.markdown = format!(
        "---\ntitle: 상태\nlocale: ko\nstyle: smartmd\n---\n\n# 상태\n\n## State\n\n{waiver}{}",
        "이 상태 모델은 별도의 과업 결과를 설명해야 하므로 UI 부재가 상태도 면제 사유가 되지 않는다."
    );
    assert!(validate(&wrong).is_err());
}

#[test]
fn trace_ids_need_real_unique_owner_headings_and_complete_reference_edges() {
    for (kind, before, after) in [
        (DocumentKind::Drivers, "### DRV-001", "DRV-001"),
        (DocumentKind::QualityScenarios, "### QA-001", "### QA-01"),
        (DocumentKind::Alternatives, "QA-001", "QA-999"),
        (DocumentKind::Decisions, "ALT-001", "대안"),
        (DocumentKind::Delivery, "ADR-001", "결정"),
    ] {
        let mut package = sample();
        replace(&mut package, kind, before, after);
        assert!(
            validate(&package).is_err(),
            "accepted {kind:?} {before}→{after}"
        );
    }
    let mut duplicate = sample();
    duplicate
        .documents
        .iter_mut()
        .find(|d| d.kind == DocumentKind::Drivers)
        .unwrap()
        .markdown
        .push_str("\n### DRV-001 중복\nQA-001 ADR-001\n");
    assert!(validate(&duplicate).is_err());
    let mut wrong_owner = sample();
    wrong_owner
        .markdown
        .push_str("\n### DRV-002 잘못된 소유자\nQA-001 ADR-001\n");
    assert!(validate(&wrong_owner).is_err());
    let mut fake = sample();
    replace(
        &mut fake,
        DocumentKind::Drivers,
        "### DRV-001 선택한 과업과 결과의 일치",
        "<!--\n### DRV-001 선택한 과업과 결과의 일치\n-->",
    );
    assert!(validate(&fake).is_err());
}

#[test]
fn cross_document_links_and_anchors_must_exist_in_server_owned_package() {
    for (before, after) in [
        ("화면설계.md#resource-list", "화면설계.md#missing-screen"),
        ("화면설계.md#resource-list", "missing.md#resource-list"),
        ("화면설계.md#resource-list", "../화면설계.md#resource-list"),
        (
            "화면설계.md#resource-list",
            "%ED%99%94%EB%A9%B4.md#resource-list",
        ),
    ] {
        let mut package = sample();
        replace(&mut package, DocumentKind::Navigation, before, after);
        assert!(validate(&package).is_err(), "accepted {after}");
    }
    let mut missing_map = sample();
    missing_map.markdown = missing_map
        .markdown
        .replace("[State](상태도.md)", "State 설명");
    assert!(validate(&missing_map).is_err());
    let mut reference = sample();
    reference.documents[0]
        .markdown
        .push_str("\n[계약][missing]\n\n[missing]: invented.md#invented\n");
    assert!(validate(&reference).is_err());
}

#[test]
fn heading_links_match_visible_text_and_normalized_korean() {
    use unicode_normalization::UnicodeNormalization;
    let mut package = sample();
    let layout = package
        .documents
        .iter_mut()
        .find(|d| d.kind == DocumentKind::ScreenLayout)
        .unwrap();
    layout.markdown.push_str(&format!("\n### {}\n\n설명.\n\n### **[화면](https://example.com)** &amp; <span>배치</span>\n\n설명.\n\n### _강조_ `원문_값`\n\n설명.\n", "한글 배치".nfd().collect::<String>()));
    package
        .documents
        .iter_mut()
        .find(|d| d.kind == DocumentKind::Navigation)
        .unwrap()
        .markdown
        .push_str("\n[한글](화면설계.md#한글-배치) [표시문](화면설계.md#화면--배치) [강조](화면설계.md#강조-원문_값)\n");
    validate(&package).unwrap();
}

#[test]
fn byte_limits_reject_one_large_document_and_a_large_bundle() {
    let mut large = sample();
    large.documents[0]
        .markdown
        .push_str(&"x".repeat(design_package::MAX_DOCUMENT_BYTES));
    assert!(validate(&large).unwrap_err().to_string().contains("150000"));
    let mut large = sample();
    for document in &mut large.documents {
        document.markdown.push_str(&"x".repeat(60_000));
    }
    assert!(validate(&large).unwrap_err().to_string().contains("500000"));
}

#[test]
fn aggregate_hash_covers_every_document_and_is_order_independent() {
    let package = sample();
    let original = design_package::package_hash(&package.markdown, &package.documents);
    let mut reordered = package.clone();
    reordered.documents.reverse();
    assert_eq!(
        original,
        design_package::package_hash(&reordered.markdown, &reordered.documents)
    );
    for index in 0..package.documents.len() {
        let mut changed = package.clone();
        changed.documents[index].markdown.push_str("\n변경");
        assert_ne!(
            original,
            design_package::package_hash(&changed.markdown, &changed.documents)
        );
    }
    assert_eq!(
        design_package::package_hash(&package.markdown, &[]),
        hash(&package.markdown)
    );
}

#[test]
fn generation_and_critic_use_package_rules_and_unabridged_companion_context() {
    let package = sample();
    let mut session = domain::new_session("example".into(), "unknown".into(), vec![]);
    let mut documents = package.documents;
    let sentinel = "검토 끝부분 보존 934f";
    documents.last_mut().unwrap().markdown.push_str(sentinel);
    session.drafts.push(Draft {
        id: id(),
        input_revision: session.design_revision,
        hash: design_package::package_hash(&package.markdown, &documents),
        markdown: package.markdown,
        documents,
        created_at: now(),
        content_status: "draft".into(),
        reviewed: false,
    });
    for kind in ["design", "critic"] {
        let prompt = domain::prompt(&session, kind, "ko").unwrap();
        assert!(prompt.contains(design_package::RULES));
        assert!(!prompt.contains("A concise single document is preferred"));
        assert!(!prompt.contains("Use Mermaid for a useful context or component diagram"));
    }
    let critic = domain::prompt(&session, "critic", "ko").unwrap();
    assert!(critic.contains(sentinel));
    for kind in DocumentKind::ALL {
        assert!(
            critic.contains(kind.filename()),
            "missing filename {}",
            kind.filename()
        );
    }
    let context: Value =
        serde_json::from_str(critic.split_once("CONTEXT_JSON:\n").unwrap().1).unwrap();
    assert_eq!(context["draft"]["documents"].as_array().unwrap().len(), 9);
    for document in context["draft"]["documents"].as_array().unwrap() {
        assert!(!document["markdown"].as_str().unwrap().is_empty());
    }
    for expected in &session.drafts[0].documents {
        let actual = context["draft"]["documents"]
            .as_array()
            .unwrap()
            .iter()
            .find(|document| document["kind"] == serde_json::to_value(expected.kind).unwrap())
            .unwrap();
        assert_eq!(actual["markdown"], expected.markdown);
        assert_eq!(actual["filename"], expected.kind.filename());
    }
    assert_eq!(context["draft"]["format"], "architecture-package-v1");
    assert!(
        !domain::prompt(&session, "conversation", "ko")
            .unwrap()
            .contains("ARCHITALK DESIGN PACKAGE")
    );
    let spec = architalk::config::Settings::default().run_spec(
        "design",
        "test".into(),
        "/tmp/test".into(),
    );
    assert_eq!(
        domain::effective_summary(&spec)["prompt_version"],
        "architalk-v4-design-package"
    );
    assert_eq!(domain::effective_summary(&spec)["schema_version"], 3);
    assert_eq!(MAIN_FILENAME, "아키텍처-설계.md");
}

#[test]
fn critic_rejects_an_oversized_context_without_truncating_companions() {
    let package = sample();
    let mut session = domain::new_session("example".into(), "unknown".into(), vec![]);
    session.messages[0].content = "x".repeat(880_000);
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
    let error = domain::prompt(&session, "critic", "ko")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("complete") && error.contains("context"),
        "{error}"
    );
}

#[test]
fn legacy_draft_review_is_labeled_without_retroactive_package_requirements() {
    let mut session = domain::new_session("old".into(), "old".into(), vec![]);
    let text = "# Old architecture\nThis was stored before the package format.";
    session.drafts.push(Draft {
        id: id(),
        input_revision: 1,
        hash: hash(text),
        markdown: text.into(),
        documents: vec![],
        created_at: now(),
        content_status: "draft".into(),
        reviewed: false,
    });
    let prompt = domain::prompt(&session, "critic", "ko").unwrap();
    let context: Value =
        serde_json::from_str(prompt.split_once("CONTEXT_JSON:\n").unwrap().1).unwrap();
    assert_eq!(context["draft"]["format"], "legacy-single-document");
    assert_eq!(context["draft"]["markdown"], text);
    assert!(prompt.contains("Do not retroactively require companion documents"));
}
