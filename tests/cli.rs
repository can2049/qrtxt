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
fn oversized_input_splits_automatically() {
    // Lowercase forces byte mode; 4000 bytes exceeds version 40-L (~2953).
    let payload = "x".repeat(4000);
    let text = stdout(&["-e", "L", &payload]);
    assert!(text.contains("QR 1/"), "missing first caption");
    assert!(text.contains("QR 2/"), "missing second caption");
}

#[test]
fn input_within_one_symbol_has_no_caption() {
    assert!(!stdout(&["hello"]).contains("QR "));
}

#[test]
fn max_bytes_splits_into_smaller_codes() {
    let payload = "a".repeat(250);
    let text = stdout(&["-e", "L", "--max-bytes", "100", &payload]);
    assert!(text.contains("QR 1/3"), "missing first caption");
    assert!(text.contains("QR 3/3"), "missing last caption");
}

#[test]
fn max_bytes_above_the_payload_keeps_a_single_code() {
    assert_eq!(
        stdout(&["--max-bytes", "1000", "hello"]),
        stdout(&["hello"])
    );
}

#[test]
fn max_bytes_zero_exits_two() {
    qrtxt()
        .args(["--max-bytes", "0", "hello"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn chunk_splits_into_at_least_the_requested_count() {
    // 100 bytes fit one symbol, but --chunk 4 forces four balanced codes.
    let payload = "a".repeat(100);
    let text = stdout(&["-e", "L", "--chunk", "4", &payload]);
    assert!(text.contains("QR 1/4"), "missing first caption");
    assert!(text.contains("QR 4/4"), "missing last caption");
}

#[test]
fn chunk_short_flag_and_zero_rejection() {
    let payload = "a".repeat(100);
    assert!(stdout(&["-c", "4", &payload]).contains("QR 4/4"));
    qrtxt()
        .args(["--chunk", "0", "hello"])
        .assert()
        .failure()
        .code(2);
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

#[test]
fn short_help_documents_the_glyph_letters() {
    let help = stdout(&["-h"]);
    assert!(help.contains("half (h"), "{help}");
    assert!(help.contains("quadrant (q"), "{help}");
    assert!(help.contains("braille (b"), "{help}");
}

#[test]
fn short_help_advertises_the_source_url() {
    let help = stdout(&["-h"]);
    assert!(help.contains("https://github.com/can2049/qrtxt"), "{help}");
}

#[test]
fn kitty_requires_a_terminal() {
    // Under the test harness stdout is a pipe, so the guard fires before any probe.
    qrtxt()
        .args(["--kitty", "hello"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("not a terminal"));
}

#[test]
fn kitty_conflicts_with_no_compact() {
    qrtxt()
        .args(["--kitty", "--no-compact", "hello"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn sixel_requires_a_terminal() {
    // Under the test harness stdout is a pipe, so the guard fires before any probe.
    qrtxt()
        .args(["--sixel", "hello"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("not a terminal"));
}

#[test]
fn sixel_conflicts_with_kitty_and_no_compact() {
    qrtxt()
        .args(["--sixel", "--kitty", "hello"])
        .assert()
        .failure()
        .code(2);
    qrtxt()
        .args(["--sixel", "--no-compact", "hello"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn short_help_lists_the_kitty_option() {
    let help = stdout(&["-h"]);
    assert!(help.contains("--kitty"), "{help}");
}

#[test]
fn short_help_lists_the_sixel_option() {
    let help = stdout(&["-h"]);
    assert!(help.contains("--sixel"), "{help}");
}
