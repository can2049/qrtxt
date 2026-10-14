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
    assert!(text.contains("QR Code 1/"), "missing first caption");
    assert!(text.contains("QR Code 2/"), "missing second caption");
}

#[test]
fn input_within_one_symbol_has_no_caption() {
    assert!(!stdout(&["hello"]).contains("QR Code "));
}

#[test]
fn max_bytes_splits_into_smaller_codes() {
    let payload = "a".repeat(250);
    let text = stdout(&["-e", "L", "--max-bytes", "100", &payload]);
    assert!(text.contains("QR Code 1/3"), "missing first caption");
    assert!(text.contains("QR Code 3/3"), "missing last caption");
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
    assert!(text.contains("QR Code 1/4"), "missing first caption");
    assert!(text.contains("QR Code 4/4"), "missing last caption");
}

#[test]
fn chunk_short_flag_and_zero_rejection() {
    let payload = "a".repeat(100);
    assert!(stdout(&["-c", "4", &payload]).contains("QR Code 4/4"));
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
    let help = stdout(&["qr", "-h"]);
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
fn kitty_mode_requires_a_terminal() {
    // Under the test harness stdout is a pipe, so the guard fires before any probe.
    qrtxt()
        .args(["--mode", "kitty", "hello"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("not a terminal"));
}

#[test]
fn sixel_mode_requires_a_terminal() {
    // Under the test harness stdout is a pipe, so the guard fires before any probe.
    qrtxt()
        .args(["--mode", "sixel", "hello"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("not a terminal"));
}

#[test]
fn auto_falls_back_to_text_without_a_terminal() {
    // A piped run must never receive bitmap escape bytes: `auto` resolves to text.
    assert_eq!(stdout(&["hello"]), stdout(&["--mode", "text", "hello"]));
}

#[test]
fn text_output_carries_an_explicit_colour_context() {
    // The block glyphs are wrapped so dark modules read black on a white field,
    // independent of the terminal's own colours (so the quiet zone stays light
    // even on a black terminal).
    let output = stdout(&["hello"]);
    assert!(output.starts_with("\x1b[30;107m"), "{output:?}");
    assert!(output.contains("\x1b[0m"), "{output:?}");
}

#[test]
fn ansi_output_carries_an_explicit_colour_context() {
    let output = stdout(&["--mode", "ansi", "hello"]);
    assert!(output.contains("\x1b[40m"), "{output:?}");
    assert!(output.contains("\x1b[107m"), "{output:?}");
    assert!(output.contains("\x1b[0m"), "{output:?}");
}

#[test]
fn unknown_mode_exits_two() {
    qrtxt()
        .args(["--mode", "dense", "hello"])
        .assert()
        .failure()
        .code(2);
}

#[test]
fn removed_switches_are_rejected() {
    for args in [
        ["--no-compact", "hello"],
        ["--kitty", "hello"],
        ["--sixel", "hello"],
    ] {
        qrtxt().args(args).assert().failure().code(2);
    }
}

#[test]
fn short_help_lists_the_mode_option_and_its_values() {
    let help = stdout(&["qr", "-h"]);
    assert!(help.contains("--mode"), "{help}");
    assert!(help.contains("kitty"), "{help}");
    assert!(help.contains("sixel"), "{help}");
}

#[test]
fn short_help_lists_the_symbology_subcommands() {
    let help = stdout(&["-h"]);
    assert!(help.contains("Encode a QR code"), "{help}");
    assert!(help.contains("Data Matrix"), "{help}");
    assert!(help.contains("dm"), "{help}"); // the datamatrix alias
    assert!(help.contains("PDF417"), "{help}");
    assert!(help.contains("pdf"), "{help}"); // the pdf417 alias
}

#[test]
fn top_help_lists_the_common_options() {
    // The shared options are global, so the top-level help lists them too.
    let help = stdout(&["-h"]);
    assert!(help.contains("--file"), "{help}");
    assert!(help.contains("--mode"), "{help}");
    assert!(help.contains("--border"), "{help}");
    assert!(help.contains("--glyphs"), "{help}");
}

#[test]
fn datamatrix_subcommand_prints_a_symbol() {
    let output = stdout(&["datamatrix", "hello"]);
    assert!(output.chars().any(is_block), "no block glyphs in output");
    assert!(output.lines().count() > 4, "output too short: {output:?}");
}

#[test]
fn datamatrix_alias_matches_the_full_name() {
    assert_eq!(stdout(&["dm", "hello"]), stdout(&["datamatrix", "hello"]));
}

#[test]
fn qr_subcommand_matches_the_bare_invocation() {
    // The argv shim injects `qr`, so the flat form and the subcommand agree.
    assert_eq!(stdout(&["qr", "hello"]), stdout(&["hello"]));
}

#[test]
fn datamatrix_splits_with_its_own_caption() {
    let payload = "a".repeat(4000);
    let text = stdout(&["datamatrix", &payload]);
    assert!(
        text.contains("Data Matrix 1/"),
        "missing datamatrix caption"
    );
    assert!(!text.contains("QR Code "), "leaked a QR caption");
}

#[test]
fn pdf417_subcommand_prints_a_symbol() {
    let output = stdout(&["pdf417", "hello"]);
    assert!(output.chars().any(is_block), "no block glyphs in output");
    assert!(output.lines().count() > 4, "output too short: {output:?}");
}

#[test]
fn pdf417_alias_matches_the_full_name() {
    assert_eq!(stdout(&["pdf", "hello"]), stdout(&["pdf417", "hello"]));
}

#[test]
fn pdf417_splits_with_its_own_caption() {
    let payload = "a".repeat(4000);
    let text = stdout(&["pdf417", &payload]);
    assert!(text.contains("PDF417 1/"), "missing pdf417 caption");
    assert!(!text.contains("QR Code "), "leaked a QR caption");
    assert!(
        !text.contains("Data Matrix "),
        "leaked a Data Matrix caption"
    );
}

#[test]
fn pdf417_short_help_lists_the_error_levels() {
    let help = stdout(&["pdf417", "-h"]);
    assert!(help.contains("--error-correction"), "{help}");
    assert!(help.contains("0 (least redundancy"), "{help}");
    assert!(help.contains("[default: 2]"), "{help}");
}

#[test]
fn pdf417_rejects_out_of_range_or_letter_error_correction() {
    for level in ["9", "H"] {
        qrtxt()
            .args(["pdf417", "-e", level, "hello"])
            .assert()
            .failure()
            .code(2);
    }
}

#[test]
fn pdf417_columns_reshape_the_symbol() {
    // The colour escapes wrap every line identically, so comparing raw line
    // lengths still orders the shapes correctly.
    let max_width = |args: &[&str]| {
        stdout(args)
            .lines()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
    };
    let payload = "a payload that spans several columns";
    let default = max_width(&["pdf417", "--mode", "text", payload]);
    let narrow = max_width(&["pdf417", "--mode", "text", "--columns", "1", payload]);
    let wide = max_width(&["pdf417", "--mode", "text", "--columns", "12", payload]);
    assert!(narrow < default, "narrow={narrow} default={default}");
    assert!(wide > default, "wide={wide} default={default}");
}

#[test]
fn pdf417_columns_short_and_alias_forms_work() {
    let long = stdout(&["pdf417", "--columns", "3", "hello"]);
    assert_eq!(stdout(&["pdf417", "-C", "3", "hello"]), long);
    assert_eq!(stdout(&["pdf417", "--cl", "3", "hello"]), long);
}

#[test]
fn pdf417_columns_reject_out_of_range_or_bad_values() {
    for columns in ["0", "31", "sixty"] {
        qrtxt()
            .args(["pdf417", "--columns", columns, "hello"])
            .assert()
            .failure()
            .code(2);
    }
}

/// Run `qrtxt` with `args`, feeding `input` on standard input, and return stdout.
fn stdin_with(args: &[&str], input: &str) -> String {
    let output = qrtxt()
        .args(args)
        .write_stdin(input)
        .output()
        .expect("runs");
    assert!(output.status.success(), "expected success: {output:?}");
    String::from_utf8(output.stdout).expect("utf-8 output")
}

#[test]
fn pdf417_reads_a_file_like_a_literal() {
    let path = std::env::temp_dir().join(format!("qrtxt-pdf417-{}.txt", std::process::id()));
    std::fs::write(&path, "hello pdf").unwrap();
    let from_file = stdout(&["pdf417", "--file", path.to_str().unwrap()]);
    let _ = std::fs::remove_file(&path);
    assert_eq!(from_file, stdout(&["pdf417", "hello pdf"]));
}

#[test]
fn pdf417_reads_stdin_like_a_literal() {
    assert_eq!(
        stdin_with(&["pdf417"], "hello pdf"),
        stdout(&["pdf417", "hello pdf"])
    );
}

#[test]
fn pdf417_preserve_newline_changes_output() {
    assert_ne!(
        stdin_with(&["pdf417", "--preserve-newline"], "hi\n"),
        stdin_with(&["pdf417"], "hi\n")
    );
}

#[test]
fn pdf417_render_modes_emit_a_symbol() {
    // text draws block glyphs; ansi draws the same grid with escape codes.
    assert!(
        stdout(&["pdf417", "--mode", "text", "hello"])
            .chars()
            .any(is_block)
    );
    let ansi = stdout(&["pdf417", "--mode", "ansi", "hello"]);
    assert!(ansi.contains("\x1b["), "{ansi:?}");
    assert!(!ansi.trim().is_empty());
}

#[test]
fn pdf417_glyph_sets_emit_output() {
    for glyph in ["half", "quadrant", "braille"] {
        let text = stdout(&["pdf417", "--mode", "text", "--glyphs", glyph, "hello"]);
        assert!(!text.trim().is_empty(), "glyph={glyph}");
    }
}

#[test]
fn pdf417_border_and_invert_change_output() {
    let base = stdout(&["pdf417", "--mode", "text", "hello"]);
    let no_border = stdout(&["pdf417", "--mode", "text", "-b", "0", "hello"]);
    assert!(no_border.lines().count() < base.lines().count());
    assert_ne!(
        stdout(&["pdf417", "--mode", "text", "--invert", "hello"]),
        base
    );
}

#[test]
fn pdf417_accepts_the_extreme_error_levels() {
    for level in ["0", "8"] {
        assert!(
            stdout(&["pdf417", "-e", level, "hello"])
                .chars()
                .any(is_block),
            "level={level}"
        );
    }
}

#[test]
fn pdf417_max_bytes_and_chunk_drive_the_captions() {
    let payload = "a".repeat(600);
    let capped = stdout(&["pdf417", "--max-bytes", "100", &payload]);
    assert!(capped.contains("PDF417 1/6"), "{capped}");
    assert!(capped.contains("PDF417 6/6"), "{capped}");
    let chunked = stdout(&["pdf417", "-c", "5", &payload]);
    assert!(chunked.contains("PDF417 5/5"), "{chunked}");
}

#[test]
fn pdf417_combines_every_option_without_conflict() {
    let text = stdout(&[
        "pdf417",
        "-e",
        "5",
        "-C",
        "6",
        "-c",
        "3",
        "--max-bytes",
        "40",
        "-i",
        "--glyphs",
        "braille",
        "--mode",
        "text",
        "-b",
        "2",
        "combine every option 混合",
    ]);
    assert!(text.contains("PDF417 3/3"), "{text}");
}

#[test]
fn removed_format_flag_is_rejected() {
    // The flat `--format` selector is gone; the subcommand chooses the format.
    qrtxt()
        .args(["qr", "--format", "dm", "hello"])
        .assert()
        .failure()
        .code(2);
}
