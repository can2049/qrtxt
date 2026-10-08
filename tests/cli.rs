//! End-to-end CLI behavior (acceptance criteria AC-1..AC-9, AC-11, AC-12).

use assert_cmd::Command;
use predicates::prelude::*;

fn qrtxt() -> Command {
    Command::cargo_bin("qrtxt").expect("binary is built")
}

fn stdout(args: &[&str]) -> String {
    let output = qrtxt().args(args).output().expect("runs");
    assert!(output.status.success(), "expected success: {output:?}");
    String::from_utf8(output.stdout).expect("utf-8 output")
}

fn piped(input: &str) -> String {
    let output = qrtxt().write_stdin(input).output().expect("runs");
    assert!(output.status.success(), "expected success: {output:?}");
    String::from_utf8(output.stdout).expect("utf-8 output")
}

fn is_block(character: char) -> bool {
    matches!(character, '\u{2588}' | '\u{2580}' | '\u{2584}')
}

#[test]
fn literal_input_prints_a_qr() {
    let output = stdout(&["hello"]);
    assert!(output.chars().any(is_block), "no block glyphs in output");
}

#[test]
fn half_glyphs_are_the_default() {
    assert_eq!(stdout(&["hello"]), stdout(&["--glyphs", "half", "hello"]));
}

#[test]
fn quadrant_and_braille_produce_output() {
    assert!(
        stdout(&["--glyphs", "quadrant", "hello"])
            .chars()
            .any(is_block)
    );
    assert!(!stdout(&["--glyphs", "braille", "hello"]).trim().is_empty());
}

#[test]
fn file_input_matches_literal() {
    let path = std::env::temp_dir().join(format!("qrtxt-test-{}.txt", std::process::id()));
    std::fs::write(&path, "hello").unwrap();
    let from_file = stdout(&["--file", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(from_file, stdout(&["hello"]));
}

#[test]
fn stdin_strips_one_trailing_newline() {
    assert_eq!(piped("hello\n"), stdout(&["hello"]));
    assert_eq!(piped("hello\r\n"), stdout(&["hello"]));
}

#[test]
fn preserve_newline_keeps_the_line_ending() {
    assert_ne!(piped("hello\n"), {
        let output = qrtxt()
            .args(["--preserve-newline"])
            .write_stdin("hello\n")
            .output()
            .unwrap();
        String::from_utf8(output.stdout).unwrap()
    });
}

#[test]
fn empty_input_exits_two() {
    qrtxt()
        .write_stdin("")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("input is empty"));
}

#[test]
fn file_and_literal_conflict_exits_two() {
    qrtxt()
        .args(["--file", "payload.txt", "hello"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn invert_changes_the_output() {
    assert_ne!(stdout(&["hello"]), stdout(&["--invert", "hello"]));
}

#[test]
fn border_zero_shrinks_the_output() {
    let default_lines = stdout(&["hello"]).lines().count();
    let zero_lines = stdout(&["-b", "0", "hello"]).lines().count();
    assert!(zero_lines < default_lines);
}

#[test]
fn oversized_input_exits_two_without_leaking_payload() {
    // Lowercase forces byte mode; 4000 bytes exceeds version 40-L (~2953).
    let secret = "x".repeat(4000);
    let output = qrtxt().args(["-e", "L", &secret]).output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(!stderr.contains(&secret), "stderr leaked the payload");
    assert!(
        !stderr.contains("xxxxxx"),
        "stderr leaked payload characters"
    );
}

#[test]
fn version_and_help_are_available() {
    qrtxt()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains("qrtxt"));
    qrtxt().arg("--help").assert().success();
}
