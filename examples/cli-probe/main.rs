//! Emits a synthetic domain prompt for the opt-in, metadata-only CLI probe.
//! The prompt contains no real project data or credentials.
use architalk::{
    domain,
    model::{Evidence, hash, id},
};
fn main() -> anyhow::Result<()> {
    let output = std::env::args_os().nth(1).ok_or_else(|| {
        anyhow::anyhow!("usage: cargo run --example cli-probe -- <synthetic-prompt-file>")
    })?;
    let intro = "혼자 macOS에서 쓸 개인 메모 CLI입니다. 메모 추가·목록·검색만 첫 버전에 넣고 삭제·동기화·로그인·외부 API는 제외합니다. 메모는 프로젝트의 JSON 파일 하나로 저장하고 원본 덮어쓰기 전 백업합니다. 일반 텍스트만 처리하고 민감 정보는 넣지 않습니다. 로컬 1만 건 이하, 검색은 1초 이내를 목표로 측정합니다. 프로세스 한 개만 쓰고 파일 손상은 백업에서 수동 복구합니다. 설치와 복구 담당자는 본인입니다. Rust로 구현합니다. 현재 범위에 맞는 작은 설계를 원합니다. 더 확인할 내용은 미정으로 남기고 첫 초안을 작성해 주세요.";
    let readme = "# 개인 메모 CLI\n개인용 메모를 JSON 파일로 저장하는 로컬 CLI. 다중 사용자·외부 연동·과금은 제외합니다.\n";
    let session = domain::new_session(
        "개인 메모 CLI".into(),
        intro.into(),
        vec![Evidence {
            id: id(),
            path: "README.md".into(),
            hash: hash(readme),
            text: readme.into(),
        }],
    );
    std::fs::write(output, domain::prompt(&session, "conversation", "ko")?)?;
    Ok(())
}
