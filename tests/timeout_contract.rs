use architalk::{
    ai::{self, Backend, RunSpec},
    config::{self, Settings},
};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[test]
fn default_and_omitted_timeouts_are_thirty_minutes_without_writing_a_file() {
    assert_eq!(Settings::default().ai.timeout_seconds, 1800);
    let directory = tempfile::tempdir().unwrap();
    let missing = directory.path().join("missing.toml");
    assert_eq!(config::load(&missing).unwrap().ai.timeout_seconds, 1800);
    assert!(!missing.exists());
    let omitted = directory.path().join("omitted.toml");
    let original = "[ai]\ndefault_backend = 'claude'\n";
    std::fs::write(&omitted, original).unwrap();
    assert_eq!(config::load(&omitted).unwrap().ai.timeout_seconds, 1800);
    assert_eq!(std::fs::read_to_string(&omitted).unwrap(), original);
}

#[test]
fn settings_accept_ten_through_thirty_six_hundred_seconds() {
    let mut settings = Settings::default();
    for seconds in [10, 300, 600, 1800, 3600] {
        settings.ai.timeout_seconds = seconds;
        assert!(settings.validate().is_ok(), "rejected {seconds} seconds");
    }
    for seconds in [0, 1, 9, 3601, u64::MAX] {
        settings.ai.timeout_seconds = seconds;
        assert_eq!(
            settings.validate().unwrap_err().to_string(),
            "timeout_seconds must be between 10 and 3600",
            "accepted or misreported {seconds} seconds"
        );
    }
}

#[test]
fn explicit_legacy_timeout_is_preserved_byte_for_byte() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("config.toml");
    let original =
        "# An explicit user preference remains authoritative.\n[ai]\ntimeout_seconds = 300\n";
    std::fs::write(&path, original).unwrap();
    assert_eq!(config::load(&path).unwrap().ai.timeout_seconds, 300);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn every_role_captures_the_timeout_at_run_creation() {
    let mut settings = Settings::default();
    settings.ai.timeout_seconds = 300;
    for role in ["conversation", "design", "critic"] {
        let before = settings.run_spec(role, "synthetic prompt".into(), "/unused".into());
        settings.ai.timeout_seconds = 3600;
        let after = settings.run_spec(role, "synthetic prompt".into(), "/unused".into());
        assert_eq!(before.timeout_seconds, 300, "{role} snapshot changed");
        assert_eq!(after.timeout_seconds, 3600, "{role} missed new setting");
        settings.ai.timeout_seconds = 300;
    }
}

async fn pre_cancelled_request(seconds: u64) -> ai::AiError {
    let request = RunSpec {
        backend: Backend::Claude,
        executable: "/nonexistent/timeout-contract-must-not-spawn".into(),
        model: None,
        effort: None,
        profile: None,
        timeout_seconds: seconds,
        prompt: "synthetic pre-cancelled boundary check".into(),
        project_root: "/nonexistent/timeout-contract-must-not-touch".into(),
    };
    let cancel = CancellationToken::new();
    cancel.cancel();
    let (events, _receiver) = mpsc::channel(1);
    ai::generate(request, cancel, events).await.unwrap_err()
}

#[tokio::test]
async fn adapter_accepts_one_second_process_tests_and_the_full_settings_interval() {
    for seconds in [1, 10, 300, 600, 1800, 3600] {
        let error = pre_cancelled_request(seconds).await;
        assert_eq!(error.code, "cancelled", "{seconds} seconds: {error:?}");
        assert!(!error.termination_unknown);
    }
}

#[tokio::test]
async fn adapter_rejects_out_of_range_values_before_any_cli_or_filesystem_work() {
    for seconds in [0, 3601, u64::MAX] {
        let error = pre_cancelled_request(seconds).await;
        assert_eq!(error.code, "invalid_settings");
        assert_eq!(error.message, "Timeout must be between 1 and 3600 seconds.");
        assert!(!error.termination_unknown);
    }
}
