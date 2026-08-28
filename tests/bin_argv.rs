                                                                                                   
                                                                                                   
                                                                                               

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::process::ExitStatusExt;
use std::process::Command;

#[test]
fn epa_bin_refuses_a_non_utf8_argument() {
    let bad = OsStr::from_bytes(&[0xFF]);
    let out = Command::new(env!("CARGO_BIN_EXE_epa"))
        .arg(bad)
        .output()
        .expect("epa spawns");
    assert!(
        out.status.signal().is_none(),
        "epa died on a signal, not a clean exit: {:?}",
        out.status
    );
    assert_eq!(
        out.status.code(),
        Some(2),
        "epa must refuse a non-UTF-8 argv with exit 2, not panic (101) or serve; got {:?}",
        out.status
    );
    assert!(out.stdout.is_empty(), "epa wrote to stdout on refusal");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("epa:"),
        "epa diagnostic missing its prefix: {stderr:?}"
    );
}

#[cfg(feature = "mcp")]
#[test]
fn epa_mcp_bin_refuses_a_non_utf8_argument() {
    let bad = OsStr::from_bytes(&[0xFF]);
    let out = Command::new(env!("CARGO_BIN_EXE_epa-mcp"))
        .arg(bad)
        .output()
        .expect("epa-mcp spawns");
    assert!(
        out.status.signal().is_none(),
        "epa-mcp died on a signal, not a clean exit: {:?}",
        out.status
    );
    assert_eq!(
        out.status.code(),
        Some(1),
        "epa-mcp must refuse a non-UTF-8 argv with exit 1, not panic (101) or serve; got {:?}",
        out.status
    );
    assert!(out.stdout.is_empty(), "epa-mcp wrote to stdout on refusal");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.starts_with("epa-mcp:"),
        "epa-mcp diagnostic missing its prefix: {stderr:?}"
    );
}
