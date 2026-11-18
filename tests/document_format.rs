use architalk::{domain, model::*};
use serde_json::json;

const SAMPLE: &str = include_str!("fixtures/architecture.smart.md");

fn candidate(markdown: &str) -> anyhow::Result<String> {
    domain::validate_design(&json!({"markdown": markdown}).to_string())
}

fn replace_section(markdown: &str, heading: &str, body: &str) -> String {
    let marker = format!("## {heading}\n");
    let (before, after) = markdown.split_once(&marker).unwrap();
    let suffix = after.find("\n## ").map_or("", |index| &after[index..]);
    format!("{before}{marker}\n{body}\n{suffix}")
}

#[test]
fn smartmd_document_survives_generation_trace_and_export_as_one_markdown() {
    assert_eq!(candidate(SAMPLE).unwrap(), SAMPLE);
    assert_eq!(candidate(&SAMPLE.replace('\n', "\r\n")).unwrap(), SAMPLE);
    let session = domain::new_session("example".into(), "show resources".into(), vec![]);
    let text = format!("{}{}", SAMPLE, domain::trace_appendix(&session));
    let draft = Draft {
        id: id(),
        input_revision: session.design_revision,
        hash: hash(&text),
        markdown: text.clone(),
        documents: vec![],
        created_at: now(),
        content_status: "draft".into(),
        reviewed: false,
    };
    let exported = domain::render_export(&session, &draft);
    assert!(exported.starts_with(&text));
    for language in ["navigation", "jobflow", "state", "layout"] {
        assert!(exported.contains(&format!("```{language}\n")));
    }
}

#[test]
fn required_diagrams_cannot_be_omitted_or_replaced_by_mermaid() {
    for language in ["navigation", "jobflow", "state", "layout"] {
        let missing = SAMPLE.replace(&format!("```{language}\n"), "```mermaid\n");
        assert!(candidate(&missing).is_err(), "accepted missing {language}");
    }
    for section in ["Navigation", "Jobflow", "State", "Screen Layout"] {
        assert!(candidate(&replace_section(SAMPLE, section, "미작성")).is_err());
    }
    let moved = replace_section(SAMPLE, "Navigation", "내용은 다른 섹션에 있습니다.")
        + "\n## Appendix\n```navigation\nHome --> Details\n```\n";
    assert!(candidate(&moved).is_err());
}

#[test]
fn examples_comments_callouts_and_table_cells_cannot_impersonate_real_sections() {
    let (before, sections) = SAMPLE.split_once("## Navigation").unwrap();
    for wrapped in [
        format!("````markdown\n## Navigation{sections}\n````\n"),
        format!("<!--\n## Navigation{sections}\n-->\n"),
        format!(":::info\n## Navigation{sections}\n:::\n"),
        "| ## Navigation | ## Jobflow | ## State | ## Screen Layout |\n|---|---|---|---|\n| code | code | code | code |\n".to_owned(),
    ] {
        assert!(candidate(&format!("{before}{wrapped}")).is_err());
    }
    let fake_attribute = format!("{before}{{table caption=\"\n## Navigation{sections}\n\"}}\n");
    assert!(candidate(&fake_attribute).is_err());
}

#[test]
fn smartmd_extensions_must_be_real_closed_blocks() {
    for invalid in [
        SAMPLE.replacen("style: smartmd", "style: plain", 1),
        SAMPLE.replacen("locale: ko", "locale: unknown", 1),
        format!("\n{SAMPLE}"),
        format!("<!-- comment -->{SAMPLE}"),
        SAMPLE.replacen(":::info", "```text\n:::info", 1),
        SAMPLE.replacen("\n:::\n", "\n", 1),
        SAMPLE.replacen("{table", "```text\n{table", 1),
        SAMPLE.replacen("| --- | --- |", "| malformed | malformed |", 1),
        SAMPLE.to_string() + "\n```text\nunclosed\n",
    ] {
        assert!(candidate(&invalid).is_err());
    }
    let without_callout = SAMPLE
        .replace(":::info title=\"설계 범위와 근거\"", "")
        .replacen("\n:::\n", "\n", 1);
    assert!(candidate(&without_callout).is_err());
}

#[test]
fn renderer_boundaries_prevent_nested_examples_and_fake_table_specs() {
    let (before, sections) = SAMPLE.split_once("## Jobflow").unwrap();
    let fake = format!("{before}```text\n```example\n```\n## Jobflow{sections}\n```\n```\n");
    assert!(
        candidate(&fake).is_err(),
        "mdview treats the missing sections as text examples"
    );
    let start = SAMPLE.find("{table ").unwrap();
    let end = SAMPLE[start..].find('\n').unwrap() + start;
    let fake = format!(
        "{}{{table caption=\"title=title=\" columns=[]}}{}",
        &SAMPLE[..start],
        &SAMPLE[end..]
    );
    assert!(
        candidate(&fake).is_err(),
        "caption strings are not column specifications"
    );
    let valid = SAMPLE.to_string() + "\n```html\n<!--\n```\n";
    candidate(&valid).unwrap();
    for invalid in [
        SAMPLE.replacen("```navigation", " ```navigation", 1),
        SAMPLE.replacen("```navigation", "``` navigation", 1),
    ] {
        assert!(candidate(&invalid).is_err());
    }
    let spaced = SAMPLE
        .replace("title=", "title = ")
        .replace("align=", "align = ");
    candidate(&spaced).unwrap();
}

#[test]
fn meaningful_dsl_structure_is_required() {
    let examples = [
        ("Navigation", "```navigation\n\n```"),
        ("Navigation", "```navigation\nA[Home] --> B[Details]\n```"),
        ("Navigation", "```navigation\n<User> --> Home\n```"),
        (
            "Jobflow",
            "```jobflow\norchestrator: Reader\nObject: Reader, Store\n```",
        ),
        (
            "Jobflow",
            "```jobflow\norchestrator: Reader\nObject: Reader\nReader.Open --> Store.Load\n```",
        ),
        (
            "Jobflow",
            "```jobflow\norchestrator: Missing\nObject: Reader, Store\nReader.Open --> Store.Load\n```",
        ),
        ("State", "```state\n<s> --> (Ready\n```"),
        ("State", "```state\nA --> B\n```"),
        ("Screen Layout", "```layout\nScreen H Main, Footer\n```"),
        ("Screen Layout", "```layout\nScreen V Main : 100%\n```"),
        (
            "Screen Layout",
            "```layout\nScreen V Main\nDetached V Other\n```",
        ),
        (
            "Screen Layout",
            "```layout\nScreen V Main\nMain V Screen\n```",
        ),
        (
            "Screen Layout",
            "```layout\nScreen V Main, Side\nMain V Item\nSide V Item\n```",
        ),
    ];
    for (heading, body) in examples {
        assert!(
            candidate(&replace_section(SAMPLE, heading, body)).is_err(),
            "accepted {heading}: {body}"
        );
    }
}

#[test]
fn explicitly_non_graphical_interfaces_have_a_scoped_exception() {
    let waiver = "Applicability: not_applicable\nReason: 이 프로젝트는 사용자 화면 없이 파일을 처리하는 배치 도구다.\nAlternative interface: CLI 입력 파일과 종료 코드로 결과를 확인하는 방식을 제안한다.";
    let non_ui = replace_section(
        &replace_section(SAMPLE, "Navigation", waiver),
        "Screen Layout",
        waiver,
    );
    candidate(&non_ui).unwrap();
    for invalid in [
        non_ui.replace("Applicability: not_applicable", "Applicability: unknown"),
        non_ui.replace("Alternative interface:", "Other:"),
        non_ui.replace("Reason:", "Other:"),
        replace_section(SAMPLE, "Jobflow", waiver),
        replace_section(SAMPLE, "State", waiver),
        replace_section(
            SAMPLE,
            "Navigation",
            &format!("{waiver}\n```navigation\nHome --> Detail\n```"),
        ),
    ] {
        assert!(candidate(&invalid).is_err());
    }
}

#[test]
fn a_single_page_without_navigation_can_explain_its_same_page_tasks() {
    let waiver = "Applicability: not_applicable\nReason: 모든 입력과 결과가 하나의 페이지에서 바뀌어 화면 이동이 없다.\nAlternative interface: [자료 목록 배치](#resource-list)의 입력과 재시도 동작을 사용한다.\n\n### 화면 확인 링크\n\n| 페이지·장면 | 화면 HTML | Layout |\n| --- | --- | --- |\n| 자료 목록·입력과 결과 | 확인 불가 — 화면 HTML 미생성 | [자료 목록 배치](#resource-list) |";
    candidate(&replace_section(SAMPLE, "Navigation", waiver)).unwrap();
}

#[test]
fn generation_and_critic_share_the_portable_contract_without_changing_conversation() {
    let session = domain::new_session("example".into(), "unknown".into(), vec![]);
    for kind in ["design", "critic"] {
        let prompt = domain::prompt(&session, kind, "ko").unwrap();
        assert!(prompt.contains(architalk::design_package::RULES));
        assert!(prompt.contains("Keep unknown facts unknown") || kind == "critic");
        assert!(!prompt.contains("/Users/"));
    }
    assert!(
        !domain::prompt(&session, "conversation", "ko")
            .unwrap()
            .contains("ARCHITALK DOCUMENT FORMAT")
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
}

#[test]
fn historical_drafts_remain_exportable_without_format_migration() {
    let session = domain::new_session("old".into(), "old source".into(), vec![]);
    let markdown = "# Existing document\nA historical architecture without SmartMD.";
    assert!(candidate(markdown).is_err());
    let draft = Draft {
        id: id(),
        input_revision: session.design_revision,
        hash: hash(markdown),
        markdown: markdown.into(),
        documents: vec![],
        created_at: now(),
        content_status: "draft".into(),
        reviewed: true,
    };
    assert!(domain::render_export(&session, &draft).starts_with(markdown));
    assert_eq!(draft.markdown, markdown);
}

#[cfg(target_os = "macos")]
#[tokio::test]
async fn invalid_design_fails_once_without_paid_repair_and_preserves_prior_drafts() {
    use architalk::{app::App, config::Settings, storage::Store};
    use std::{os::unix::fs::PermissionsExt, time::Duration};
    for has_previous in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let external = tempfile::tempdir().unwrap();
        let script = include_str!("fixtures/claude-workflow.py");
        let executable = external.path().join("fixture.py");
        std::fs::write(&executable, script).unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o700)).unwrap();
        // A long architecture was accepted by the previous length-only rule.
        std::fs::write(
            external.path().join("architecture.smart.md"),
            format!(
                "# Unsupported design\n{}",
                "missing dedicated design sections. ".repeat(20)
            ),
        )
        .unwrap();
        let mut settings = Settings::default();
        settings.ai.backends.claude.executable = executable.to_str().unwrap().into();
        let mut session = domain::new_session("fixture".into(), "file tool".into(), vec![]);
        let prior = "# Historical draft\nKeep the exact old content.";
        if has_previous {
            session.drafts.push(Draft {
                id: id(),
                input_revision: session.design_revision,
                hash: hash(prior),
                markdown: prior.into(),
                documents: vec![],
                created_at: now(),
                content_status: "draft".into(),
                reviewed: true,
            });
        }
        let sid = session.id.clone();
        let mut workspace = Workspace::new(settings.clone());
        workspace.sessions.push(session);
        let mut store = Store::open(root.path()).unwrap();
        store
            .commit_workspace(&serde_json::to_value(workspace).unwrap(), None, None)
            .unwrap();
        drop(store);
        let app = App::open(root.path(), settings).unwrap();
        let mut events = app.events.subscribe();
        app.command(
            "draft",
            Some(&sid),
            Command {
                command_id: id(),
                expected_revision: 0,
                payload: json!({}),
            },
        )
        .unwrap();
        tokio::time::timeout(Duration::from_secs(15), async {
            while !app.is_idle() {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .unwrap();
        let workspace = app.workspace().unwrap();
        assert_eq!(
            workspace["runs"].as_array().unwrap().len(),
            1,
            "no follow-up critic or repair run"
        );
        assert_eq!(workspace["runs"][0]["status"], "failed");
        assert_eq!(
            workspace["runs"][0]["error"]["code"],
            "invalid_model_output"
        );
        // Each real fixture generation emits one verified initialization event.
        // Count it in the parent; the CLI must not write a shared counter file.
        let mut attempts = 0;
        while let Ok(event) = events.try_recv() {
            if event.kind == "progress"
                && event.payload["message"] == "Claude is generating a response."
            {
                attempts += 1;
            }
        }
        assert_eq!(
            attempts, 1,
            "one CLI generation attempt without paid repair"
        );
        let session = app.session(&sid).unwrap();
        assert_eq!(
            session["drafts"].as_array().unwrap().len(),
            usize::from(has_previous)
        );
        if has_previous {
            assert_eq!(session["drafts"][0]["markdown"], prior);
        }
        assert!(session["active_run_id"].is_null());
    }
}
