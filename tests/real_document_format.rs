use architalk::{ai, config::Settings, domain, model};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "uses authenticated local Codex and model quota; one explicit design attempt"]
async fn real_codex_returns_a_method_r_design_package() {
    // Replay an explicitly supplied synthetic response without another model
    // request when investigating a validator failure. Never mutate that input.
    let retained = std::env::var_os("ARCHITALK_PACKAGE_RESPONSE_JSON");
    let (text, provenance) = if let Some(path) = retained {
        (
            std::fs::read_to_string(path).unwrap(),
            serde_json::json!({"mode":"retained-synthetic-response", "model_calls":0}),
        )
    } else {
        let project = tempfile::tempdir().unwrap();
        let session = domain::new_session(
            "합성 자료 열람 웹 도구".into(),
            "개인용 자료 열람 웹 도구를 설계한다. 목록 화면에서 자료를 선택하면 상세 화면이 열리고 목록으로 복귀할 수 있다. 조회 실패는 목록에서 오류를 보이고 재시도한다. 로컬 JSON 자료를 읽기만 하며 수정·로그인·과금·외부 연동은 제외한다. 한 사람이 로컬에서 사용한다. 부하와 배포 환경은 미정이다. 확정하지 않은 정책은 미정으로 남기고 Method-R 구조, 드라이버, 품질 시나리오, 대안과 트레이드오프, ADR, 구현·운영 검증 및 네비게이션·잡플로우·상태도·화면설계를 종류별로 작성한다. 작은 프로젝트여도 중요한 선택의 근거와 조건부 대안·검증 계획을 생략하지 않는다.".into(),
            vec![],
        );
        let mut settings = Settings::default();
        settings.ai.default_backend = ai::Backend::Codex;
        settings.ai.backends.codex.effort = Some("low".into());
        settings.ai.timeout_seconds = 300;
        let spec = settings.run_spec(
            "design",
            domain::prompt(&session, "design", "ko").unwrap(),
            project.path().to_owned(),
        );
        let (tx, _rx) = mpsc::channel(64);
        let output = ai::generate(spec, CancellationToken::new(), tx)
            .await
            .unwrap();
        (
            output.text,
            serde_json::json!({"mode":"new-codex-generation", "model_calls":1,
                "cli_version":output.cli_version, "resolved_model":output.resolved_model,
                "requested_model":"cli_default", "effort":"low", "timeout_seconds":300}),
        )
    };
    let artifacts = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target/document-probes")
        .join(model::id());
    std::fs::create_dir_all(&artifacts).unwrap();
    // Synthetic input only. Retain the response if strict validation rejects it.
    std::fs::write(artifacts.join("response.json"), &text).unwrap();
    std::fs::write(
        artifacts.join("provenance.json"),
        serde_json::to_vec_pretty(&provenance).unwrap(),
    )
    .unwrap();
    let package = domain::validate_design_package(&text).unwrap();
    assert_eq!(package.documents.len(), 9);
    assert!(package.markdown.contains("## Method-R macro design"));
    std::fs::write(artifacts.join("아키텍처-설계.md"), &package.markdown).unwrap();
    let mut bytes = package.markdown.len();
    for document in &package.documents {
        bytes += document.markdown.len();
        std::fs::write(artifacts.join(document.kind.filename()), &document.markdown).unwrap();
    }
    println!(
        "{} passed Method-R package validation: 10 documents, {} bytes; artifacts target/document-probes/{}",
        provenance["mode"],
        bytes,
        artifacts.file_name().unwrap().to_string_lossy()
    );
}
