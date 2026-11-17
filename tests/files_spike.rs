use architalk::files::{ExportStatus, apply, preview, reserve};
use std::{fs, os::unix::fs::symlink};
#[test]
fn p0_real_filesystem_no_clobber_and_no_symlink_escape() {
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), root.path().join("docs")).unwrap();
    assert!(preview(root.path(), "테스트", "body", 1, "session").is_err());
    fs::remove_file(root.path().join("docs")).unwrap();
    let p = preview(root.path(), "테스트", "body", 1, "session").unwrap();
    let reserved = reserve(root.path(), &p).unwrap();
    assert!(reserve(root.path(), &p).is_err());
    let target = root
        .path()
        .join(&p.relative_directory)
        .join("아키텍처-설계.md");
    fs::write(&target, "external").unwrap();
    assert_eq!(
        apply(root.path(), &reserved).unwrap().status,
        ExportStatus::Conflict
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "external");
    assert!(
        !target
            .parent()
            .unwrap()
            .join(".architalk-export.json")
            .exists()
    );
    fs::remove_file(&target).unwrap();
    assert_eq!(
        apply(root.path(), &reserved).unwrap().status,
        ExportStatus::Completed
    );
    assert_eq!(
        apply(root.path(), &reserved).unwrap().status,
        ExportStatus::Completed
    );
    assert!(fs::read_dir(outside.path()).unwrap().next().is_none());
}
