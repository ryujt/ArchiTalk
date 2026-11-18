//! Conversation validation uses a real restricted CLI subprocess, never a model.
#[cfg(target_os = "macos")]
mod macos {
    use architalk::{
        app::App,
        config::Settings,
        domain,
        model::{Command, Draft, Workspace, hash, id, now},
        storage::Store,
    };
    use serde_json::{Value, json};
    use std::{fs, os::unix::fs::PermissionsExt, path::Path, sync::Arc, time::Duration};

    const OPTIONS_ERROR: &str = "questions[0].options: observed 5 items; limit 4 items";
    const TEXT_ERROR: &str = "questions[0].text: observed 3001 UTF-8 bytes; limit 2000 UTF-8 bytes";
    const CORRECTED_REPLY: &str = "기존 요구를 보존하고 질문을 간결하게 수정했습니다.";
    const INVALID_REPLY: &str = "INVALID_OUTPUT_MUST_NEVER_BE_COMMITTED";

    fn candidate(source: &str, quote: &str, fault: &str) -> Value {
        let mut question = json!({
            "target_ref":"scope", "text":"첫 버전의 알림 범위는 어디까지인가요?",
            "reason":"첫 구현 범위를 정하기 위해서입니다.",
            "options":["로컬 화면", "로컬 화면과 파일"], "priority_class":3
        });
        match fault {
            "options" => question["options"] = json!(["하나", "둘", "셋", "넷", "다섯"]),
            "text" => question["text"] = json!(format!("{}?", "가".repeat(1000))),
            "none" => (),
            _ => panic!("unknown synthetic fault"),
        }
        json!({
            "reply":if fault == "none" { CORRECTED_REPLY } else { INVALID_REPLY },
            "claims":[{"text":"알림은 로컬 화면에 표시한다는 제안입니다.",
                "kind":"proposal", "evidence_refs":[source], "source_quote":quote}],
            "questions":[question]
        })
    }

    fn fixture(dir: &Path, first: &Value, second: &Value, expected_error: &str) -> String {
        let executable = dir.join("question-cli.py");
        // Data is read-only to the child. Only product-owned runtime is writable;
        // assertions in the child prove the exact correction prompt it receives.
        fs::write(
            dir.join("responses.json"),
            json!({
                "first":first, "second":second, "error":expected_error
            })
            .to_string(),
        )
        .unwrap();
        fs::write(&executable, r#"#!/usr/bin/python3
import json,sys
from pathlib import Path
if '--version' in sys.argv:
    print('2.1.326 (Claude Code)');sys.exit(0)
if '--help' in sys.argv:
    print('--print --output-format --include-partial-messages --safe-mode --restricted --strict-mcp-config --mcp-config --setting-sources --settings --tools --permission-mode --disable-slash-commands --no-session-persistence --effort');sys.exit(0)
prompt=sys.stdin.read()
data=json.loads(Path(__file__).with_name('responses.json').read_text(encoding='utf-8'))
marker='\nYour previous final output failed validation: '
if marker in prompt:
    assert prompt.count(marker)==1, 'only one correction suffix is allowed'
    original,correction=prompt.split(marker)
    expected=data['error']+'. Return the complete corrected JSON only. Do not change unknown facts.\nPREVIOUS_OUTPUT:\n'
    assert correction.startswith(expected), 'precise field error must be in correction prompt'
    previous=correction[len(expected):]
    assert json.loads(previous)==data['first'], 'complete first response must be supplied unchanged'
    context=json.loads(original.split('CONTEXT_JSON:\n',1)[1])
    assert context['messages'][-1]['role']=='user', 'original user context must be preserved'
    result=data['second']
else:
    assert 'TASK:\nHelp the user design' in prompt
    result=data['first']
text=json.dumps(result,ensure_ascii=False)
print(json.dumps({'type':'system','subtype':'init','tools':[],'mcp_servers':[],'permissionMode':'dontAsk','model':'question-fixture'}),flush=True)
print(json.dumps({'type':'assistant','message':{'content':[{'type':'text','text':text}]}},ensure_ascii=False),flush=True)
print(json.dumps({'type':'result','subtype':'success','is_error':False,'result':text},ensure_ascii=False),flush=True)
"#).unwrap();
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700)).unwrap();
        executable.to_str().unwrap().into()
    }

    async fn settled(app: &Arc<App>) {
        tokio::time::timeout(Duration::from_secs(15), async {
            while !app.is_idle() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("synthetic subprocess must finish");
    }

    fn content(mut session: Value) -> Value {
        // Completion advances only control bookkeeping when validation fails.
        for key in ["revision", "active_run_id", "cursor", "readiness_issues"] {
            session.as_object_mut().unwrap().remove(key);
        }
        session
    }

    async fn run_case(first_fault: &str, second_fault: &str, first_error: &str) {
        let project = tempfile::tempdir().unwrap();
        let tools = tempfile::tempdir().unwrap();
        let mut session = domain::new_session(
            "질문 보정 합성 과업".into(),
            "알림을 로컬 화면에서 확인합니다.".into(),
            vec![],
        );
        let initial_source = session.messages[0].id.clone();
        let initial_quote = session.messages[0].content.clone();
        // Start with a previously committed interpretation, pending question,
        // and historical draft, so failure cannot pass by leaving empty arrays.
        domain::apply_conversation(&mut session, serde_json::from_value(json!({
            "reply":"기존 알림 요구를 기록했습니다.",
            "claims":[{"text":"로컬 화면에서 알림을 확인한다는 해석입니다.",
                "kind":"interpretation", "evidence_refs":[initial_source], "source_quote":initial_quote}],
            "questions":[{"target_ref":"scope", "text":"첫 범위는 어디까지인가요?",
                "reason":"범위를 확인하기 위해서입니다.", "options":["화면만", "파일도"], "priority_class":3}]
        })).unwrap()).unwrap();
        let previous = "# 이전 설계\n기존 초안 바이트를 그대로 보존합니다.";
        session.drafts.push(Draft {
            id: id(),
            input_revision: session.design_revision,
            markdown: previous.into(),
            documents: vec![],
            hash: hash(previous),
            created_at: now(),
            content_status: "draft".into(),
            reviewed: true,
        });
        let sid = session.id.clone();
        let revision = session.revision;
        let first = candidate(&initial_source, &initial_quote, first_fault);
        let second = candidate(&initial_source, &initial_quote, second_fault);
        let mut settings = Settings::default();
        settings.ai.backends.claude.executable =
            fixture(tools.path(), &first, &second, first_error);
        settings.ai.backends.codex.executable = "/nonexistent/never-call-real-codex".into();
        let mut workspace = Workspace::new(settings.clone());
        workspace.sessions.push(session);
        let mut store = Store::open(project.path()).unwrap();
        store
            .commit_workspace(&serde_json::to_value(workspace).unwrap(), None, None)
            .unwrap();
        drop(store);
        let app = App::open(project.path(), settings.clone()).unwrap();
        let mut events = app.events.subscribe();
        let started = app.command("message", Some(&sid), Command {
            command_id: id(), expected_revision: revision,
            payload: json!({"content":"원래 요구는 유지하고 알림 범위를 더 확인해 주세요."}),
        }).unwrap();
        let rid = started["run"]["id"].as_str().unwrap();
        let committed_input = content(started["session"].clone());
        settled(&app).await;
        let run = app.run(rid).unwrap();
        let final_session = app.session(&sid).unwrap();
        let mut attempts = 0;
        while let Ok(event) = events.try_recv() {
            if event.kind == "progress"
                && event.payload["message"] == "Claude is generating a response."
            {
                attempts += 1;
            }
        }
        assert_eq!(
            attempts, 2,
            "one initial generation and exactly one correction; run={run}"
        );
        assert_eq!(
            app.workspace().unwrap()["runs"].as_array().unwrap().len(),
            1
        );
        assert_eq!(run["process_termination"], "confirmed");
        assert!(final_session["active_run_id"].is_null());
        if second_fault == "none" {
            assert_eq!(run["status"], "completed", "{run}");
            assert!(run["error"].is_null());
            let messages = final_session["messages"].as_array().unwrap();
            assert_eq!(
                messages.len(),
                committed_input["messages"].as_array().unwrap().len() + 1
            );
            assert!(
                messages.last().unwrap()["content"]
                    .as_str()
                    .unwrap()
                    .starts_with(CORRECTED_REPLY)
            );
            let question = final_session["questions"]
                .as_array()
                .unwrap()
                .iter()
                .find(|q| q["status"] == "pending" && q["target_ref"] == "scope")
                .unwrap();
            assert_eq!(question["text"], second["questions"][0]["text"]);
            assert_eq!(question["options"], second["questions"][0]["options"]);
            assert!(!final_session.to_string().contains(INVALID_REPLY));
            assert_eq!(final_session["drafts"], committed_input["drafts"]);
        } else {
            assert_eq!(run["status"], "failed");
            assert_eq!(run["error"]["code"], "invalid_model_output", "{run}");
            assert_eq!(run["error"]["message"], TEXT_ERROR);
            assert_eq!(
                content(final_session.clone()),
                committed_input,
                "failed candidate must not mutate any already committed session content"
            );
        }
        let persisted = content(final_session);
        drop(app);
        let reopened = App::open(project.path(), settings).unwrap();
        assert_eq!(content(reopened.session(&sid).unwrap()), persisted);
        assert_eq!(reopened.run(rid).unwrap(), run);
        assert!(reopened.is_idle());
    }

    #[tokio::test]
    async fn excess_question_options_receive_one_precise_repair_then_complete() {
        run_case("options", "none", OPTIONS_ERROR).await;
    }

    #[tokio::test]
    async fn multibyte_question_text_receives_one_byte_precise_repair_then_completes() {
        run_case("text", "none", TEXT_ERROR).await;
    }

    #[tokio::test]
    async fn failed_question_repair_stops_after_two_attempts_and_preserves_committed_state() {
        run_case("options", "text", OPTIONS_ERROR).await;
    }
}
