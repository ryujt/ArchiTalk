//! Explicit, small real conversation contract check; no user project input.
#![cfg(target_os = "macos")]
use architalk::{
    app::App,
    config::Settings,
    model::{Command, id},
};
use serde_json::json;
use std::time::Duration;

#[tokio::test]
#[ignore = "uses authenticated Claude with CLI-default model/effort; one conversation and at most one existing repair"]
async fn real_claude_first_conversation_satisfies_the_question_contract() {
    let project = tempfile::tempdir().unwrap();
    let mut settings = Settings::default();
    settings.ai.timeout_seconds = 90;
    let app = App::open(project.path(), settings).unwrap();
    let created=app.command("create",None,Command {
        command_id:id(),expected_revision:0,
        payload:json!({"title":"합성 산책 기록 도구","intro":"혼자 쓰는 산책 기록 도구를 만들고 싶습니다. 날짜와 거리 기록이 필요하며 첫 버전의 범위와 저장 방식은 아직 정하지 않았습니다. 다음에 확인할 중요한 사항을 한 가지씩 물어봐 주세요.","context_paths":[]}),
    }).unwrap();
    let sid = created["session"]["id"].as_str().unwrap();
    tokio::time::timeout(Duration::from_secs(190), async {
        while !app.is_idle() {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    })
    .await
    .unwrap();
    let workspace = app.workspace().unwrap();
    assert_eq!(workspace["runs"].as_array().unwrap().len(), 1);
    let run = &workspace["runs"][0];
    assert_eq!(
        run["status"], "completed",
        "conversation failure: {}",
        run["error"]
    );
    let session = app.session(sid).unwrap();
    assert_eq!(
        session["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "user")
            .count(),
        1
    );
    assert_eq!(
        session["messages"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|m| m["role"] == "assistant")
            .count(),
        1
    );
    let question = session["questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|q| q["id"] == session["last_question_id"])
        .unwrap();
    assert!(question["options"].as_array().unwrap().len() <= 4);
    assert!(question["text"].as_str().unwrap().len() <= 2000);
    println!(
        "actual Claude first conversation completed with CLI-default model/effort, one visible question and {} options; version={}",
        question["options"].as_array().unwrap().len(),
        run["cli_version"]
    );
}
