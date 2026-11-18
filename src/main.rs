use anyhow::{Context, Result, bail};
use architalk::{
    app::App,
    config,
    http::{self, ServerState},
};
use clap::{Parser, Subcommand};
use std::{net::Ipv4Addr, path::PathBuf, time::Duration};
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[arg(long, global = true)]
    project: Option<PathBuf>,
    #[arg(long, global = true)]
    config: Option<PathBuf>,
    #[arg(long)]
    no_open: bool,
    #[arg(long, default_value_t = 0)]
    port: u16,
    #[command(subcommand)]
    command: Option<Commands>,
}
#[derive(Subcommand)]
enum Commands {
    /// Check installed AI CLI versions/options without model calls.
    Doctor,
}
#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "architalk=info".into()),
        )
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let project = cli
        .project
        .unwrap_or(std::env::current_dir()?)
        .canonicalize()
        .context("project directory not found")?;
    if !project.is_dir() {
        bail!("--project must identify a directory");
    }
    let config_path = cli.config.unwrap_or_else(config::default_path);
    let settings = config::load(&config_path)?;
    if matches!(cli.command, Some(Commands::Doctor)) {
        let _store = architalk::storage::Store::open(&project)?;
        let (c, x) = tokio::join!(
            architalk::ai::diagnose_in_project(
                architalk::ai::Backend::Claude,
                &settings.ai.backends.claude.executable,
                &project
            ),
            architalk::ai::diagnose_in_project(
                architalk::ai::Backend::Codex,
                &settings.ai.backends.codex.executable,
                &project
            )
        );
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"diagnostics":[c,x]}))?
        );
        return Ok(());
    }
    let app = App::open_with_shared_config(&project, &config_path)?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, cli.port)).await?;
    let authority = listener.local_addr()?.to_string();
    let token = format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    );
    let url = format!("http://{authority}/#token={token}");
    println!(
        "ArchiTalk · {}\n{}\n대화 상태: .architalk/ (Git 제외 권장)\n종료: Ctrl+C",
        project.display(),
        url
    );
    if !cli.no_open {
        if let Err(e) = open::that_detached(&url) {
            tracing::warn!(error=%e,"browser could not open; use the printed local URL");
        }
    }
    let state = ServerState {
        app: app.clone(),
        token,
        authority,
    };
    let signal_app = app.clone();
    let shutdown = async move {
        #[cfg(unix)]
        {
            let mut term =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("signal handler");
            tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
        }
        #[cfg(not(unix))]
        let _ = tokio::signal::ctrl_c().await;
        signal_app.stop();
        let stop = async {
            for _ in 0..70 {
                if signal_app.is_idle() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        };
        tokio::select! {_=stop=>{},_=tokio::signal::ctrl_c()=>{}}
    };
    axum::serve(listener, http::router(state))
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}
