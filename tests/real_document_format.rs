use architalk::{ai, config::Settings, domain, model};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "uses authenticated local Codex and model quota; one explicit design attempt"]
async fn real_codex_returns_a_smartmd_design_with_all_four_diagrams() {
    let project = tempfile::tempdir().unwrap();
    let session = domain::new_session(
        "합성 자료 열람 웹 도구".into(),
        "개인용 자료 열람 웹 도구를 설계한다. 목록 화면에서 자료를 선택하면 상세 화면이 열리고 목록으로 복귀할 수 있다. 조회 실패는 목록에서 오류를 보이고 재시도한다. 로컬 JSON 자료를 읽기만 하며 수정·로그인·과금·외부 연동은 제외한다. 한 사람이 로컬에서 사용한다. 부하와 배포 환경은 미정이다. 확정하지 않은 정책은 미정으로 남기고 네비게이션·객체 협력·조회 상태·목록과 상세 화면 배치를 포함한 간결한 첫 설계를 작성한다.".into(),
        vec![],
    );
    let mut settings = Settings::default();
    settings.ai.default_backend = ai::Backend::Codex;
    settings.ai.backends.codex.effort = Some("low".into());
    let spec = settings.run_spec(
        "design",
        domain::prompt(&session, "design", "ko").unwrap(),
        project.path().to_owned(),
    );
    let (tx, _rx) = mpsc::channel(64);
    let output = ai::generate(spec, CancellationToken::new(), tx)
        .await
        .unwrap();
    let artifacts = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/document-probes")
        .join(model::id());
    std::fs::create_dir_all(&artifacts).unwrap();
    // Synthetic input only. Retain the response if strict validation rejects it.
    std::fs::write(artifacts.join("response.json"), &output.text).unwrap();
    let markdown = domain::validate_design(&output.text).unwrap();
    for language in ["navigation", "jobflow", "state", "layout"] {
        assert!(markdown.contains(&format!("```{language}\n")));
    }
    std::fs::write(artifacts.join("document.md"), &markdown).unwrap();
    println!(
        "one Codex design passed SmartMD validation: {} bytes; artifacts target/document-probes/{}",
        markdown.len(),
        artifacts.file_name().unwrap().to_string_lossy()
    );
}
