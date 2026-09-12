use super::MAX_OUTPUT_BYTES;
use super::read_output;
use pretty_assertions::assert_eq;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

#[test]
fn captures_successful_probe_output() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 'TERM_PROGRAM=vscode\\r\\n'"]);
    assert_eq!(
        read_output(&mut command, Duration::from_secs(2)).unwrap(),
        Some(b"TERM_PROGRAM=vscode\r\n".to_vec())
    );
}

#[test]
fn unsuccessful_probe_does_not_report_a_terminal() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf 'TERM_PROGRAM=vscode'; exit 1"]);
    assert_eq!(
        read_output(&mut command, Duration::from_secs(2)).unwrap(),
        None
    );
}

#[test]
fn hanging_probe_is_terminated() {
    let mut command = Command::new("/bin/sleep");
    command.arg("30");
    let start = Instant::now();
    assert_eq!(
        read_output(&mut command, Duration::from_millis(20)).unwrap(),
        None
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}

#[test]
fn excessive_output_does_not_block_on_a_full_pipe() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "while :; do printf 'TERM_PROGRAM=vscode\\n'; done"]);
    command.stderr(Stdio::null());
    assert_eq!(
        read_output(&mut command, Duration::from_secs(2)).unwrap(),
        None
    );
}

#[test]
fn output_at_the_limit_is_preserved() {
    let expected = "x".repeat(MAX_OUTPUT_BYTES);
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf '%s' \"$1\"", "probe", &expected]);
    assert_eq!(
        read_output(&mut command, Duration::from_secs(2)).unwrap(),
        Some(expected.into_bytes())
    );
}

#[test]
fn retained_stdout_does_not_wait_for_eof() {
    // The shell exits immediately; its short-lived descendant retains stdout.
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "sleep 3 & printf 'TERM_PROGRAM=vscode'"]);
    let start = Instant::now();
    assert_eq!(
        read_output(&mut command, Duration::from_secs(2)).unwrap(),
        Some(b"TERM_PROGRAM=vscode".to_vec())
    );
    assert!(start.elapsed() < Duration::from_secs(2));
}
