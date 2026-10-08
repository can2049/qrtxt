//! Round-trip verification (AC-10, AC-13, AC-14): the rendered terminal text is
//! parsed back into the module grid and decoded with `rqrr`; it must equal the
//! original payload. This proves the printed code is still scannable.
//!
//! The parser recovers the raw *ink* grid (what the terminal draws in the
//! foreground). Because the quiet zone is background in every mode, its ink is
//! `false` and cannot be told apart from dark modules by ink alone; the decoder
//! therefore delimits the symbol using the known default border (4) and rebuilds
//! a canonical image with a white quiet zone before decoding.

use assert_cmd::Command;
use rqrr::PreparedImage;

const DEFAULT_BORDER: usize = 4;
const MAGNIFICATION: usize = 8;
const CANONICAL_MARGIN: usize = 4;

const QUADRANT: [char; 16] = [
    ' ', '\u{2598}', '\u{259D}', '\u{2580}', '\u{2596}', '\u{258C}', '\u{259E}', '\u{259B}',
    '\u{2597}', '\u{259A}', '\u{2590}', '\u{259C}', '\u{2584}', '\u{2599}', '\u{259F}', '\u{2588}',
];

const BRAILLE_WEIGHTS: [[u32; 4]; 2] = [[0x01, 0x02, 0x04, 0x40], [0x08, 0x10, 0x20, 0x80]];

/// CJK and mixed-script payloads that must survive a full encode/decode round
/// trip unchanged. Entry 5 is the byte pattern that was previously mis-encoded
/// as Shift-JIS Kanji; entry 6 mixes scripts, ASCII, digits and emoji.
const CJK_SAMPLES: &[&str] = &[
    "简体中文：你好，世界！",
    "繁體中文：你好，世界！",
    "ひらがな カタカナ 漢字 の テスト",
    "한국어 테스트 안녕하세요",
    "反相）,再按字形集把网格打包成字符。",
    "混合 mixed ABC 123 한자 漢字 😀 ，。！？「」《》；：",
    "参数 file 文件 值 = 42",
];

fn tile_width(glyph: &str) -> usize {
    match glyph {
        "half" => 1,
        "quadrant" | "braille" => 2,
        other => panic!("unknown glyph set {other}"),
    }
}

fn render(args: &[&str]) -> String {
    let output = Command::cargo_bin("qrtxt")
        .expect("binary is built")
        .args(args)
        .output()
        .expect("runs");
    assert!(output.status.success(), "qrtxt failed: {output:?}");
    String::from_utf8(output.stdout).expect("utf-8 output")
}

/// Parse the rendered text into a raw ink grid `(width, height, ink)`.
fn parse(text: &str, glyph: &str) -> (usize, usize, Vec<bool>) {
    match glyph {
        "half" => parse_half(text),
        "quadrant" => parse_quadrant(text),
        "braille" => parse_braille(text),
        other => panic!("unknown glyph set {other}"),
    }
}

fn parse_half(text: &str) -> (usize, usize, Vec<bool>) {
    let rows: Vec<&str> = text.lines().collect();
    let width = rows[0].chars().count();
    let height = rows.len() * 2;
    let mut ink = vec![false; width * height];
    for (row, line) in rows.iter().enumerate() {
        for (x, character) in line.chars().enumerate() {
            let (top, bottom) = match character {
                '\u{2588}' => (true, true),
                '\u{2580}' => (true, false),
                '\u{2584}' => (false, true),
                ' ' => (false, false),
                other => panic!("unexpected half glyph {other:?}"),
            };
            ink[row * 2 * width + x] = top;
            ink[(row * 2 + 1) * width + x] = bottom;
        }
    }
    (width, height, ink)
}

fn parse_quadrant(text: &str) -> (usize, usize, Vec<bool>) {
    let rows: Vec<&str> = text.lines().collect();
    let width = rows[0].chars().count() * 2;
    let height = rows.len() * 2;
    let mut ink = vec![false; width * height];
    for (row, line) in rows.iter().enumerate() {
        for (column, character) in line.chars().enumerate() {
            let index = QUADRANT
                .iter()
                .position(|&candidate| candidate == character)
                .unwrap_or_else(|| panic!("unexpected quadrant glyph {character:?}"));
            let x = column * 2;
            let y = row * 2;
            ink[y * width + x] = index & 1 != 0;
            ink[y * width + x + 1] = index & 2 != 0;
            ink[(y + 1) * width + x] = index & 4 != 0;
            ink[(y + 1) * width + x + 1] = index & 8 != 0;
        }
    }
    (width, height, ink)
}

fn parse_braille(text: &str) -> (usize, usize, Vec<bool>) {
    let rows: Vec<&str> = text.lines().collect();
    let width = rows[0].chars().count() * 2;
    let height = rows.len() * 4;
    let mut ink = vec![false; width * height];
    for (row, line) in rows.iter().enumerate() {
        for (column, character) in line.chars().enumerate() {
            let bits = character as u32 - 0x2800;
            for (dot_column, weights) in BRAILLE_WEIGHTS.iter().enumerate() {
                for (dot_row, weight) in weights.iter().enumerate() {
                    let x = column * 2 + dot_column;
                    let y = row * 4 + dot_row;
                    ink[y * width + x] = bits & weight != 0;
                }
            }
        }
    }
    (width, height, ink)
}

/// Rebuild a canonical black-on-white image (white quiet zone) and decode it.
///
/// `ink` covers `(parsed_width, parsed_height)` modules; the symbol itself sits
/// at offset `DEFAULT_BORDER` with size `size x size`.
fn decode(
    parsed_width: usize,
    parsed_height: usize,
    ink: &[bool],
    size: usize,
    invert: bool,
) -> String {
    let side = size + 2 * CANONICAL_MARGIN;
    let is_dark = |x: usize, y: usize| -> bool {
        if x >= CANONICAL_MARGIN
            && x < CANONICAL_MARGIN + size
            && y >= CANONICAL_MARGIN
            && y < CANONICAL_MARGIN + size
        {
            let source = (y - CANONICAL_MARGIN + DEFAULT_BORDER) * parsed_width
                + (x - CANONICAL_MARGIN + DEFAULT_BORDER);
            let cell_ink = ink[source];
            // Default: light modules are ink; inverted: dark modules are ink.
            if invert { cell_ink } else { !cell_ink }
        } else {
            false // canonical quiet zone is light
        }
    };
    let _ = parsed_height;
    let mut image = PreparedImage::prepare_from_greyscale(
        side * MAGNIFICATION,
        side * MAGNIFICATION,
        |px, py| {
            if is_dark(px / MAGNIFICATION, py / MAGNIFICATION) {
                0
            } else {
                255
            }
        },
    );
    let grids = image.detect_grids();
    assert_eq!(grids.len(), 1, "expected exactly one QR grid");
    let (_meta, content) = grids[0].decode().expect("decodes");
    content
}

fn symbol_size(parsed_width: usize, glyph: &str) -> usize {
    // QR symbol sizes are odd; tile padding may add one column for 2-wide glyphs.
    let size = parsed_width - 2 * DEFAULT_BORDER - (tile_width(glyph) - 1);
    assert_eq!(size % 2, 1, "unexpected symbol size from parsed width");
    size
}

fn round_trip(payload: &str, glyph: &str, invert: bool) -> String {
    let mut args = vec!["--glyphs", glyph];
    if invert {
        args.push("--invert");
    }
    args.push(payload);
    let text = render(&args);
    let (width, height, ink) = parse(&text, glyph);
    let size = symbol_size(width, glyph);
    decode(width, height, &ink, size, invert)
}

/// Like [`round_trip`], but with extra flags and the payload passed after `--`,
/// so payloads containing option-like tokens are never parsed as options.
fn round_trip_with(payload: &str, glyph: &str, invert: bool, extra: &[&str]) -> String {
    let mut args = vec!["--glyphs", glyph];
    if invert {
        args.push("--invert");
    }
    args.extend_from_slice(extra);
    args.push("--");
    args.push(payload);
    let text = render(&args);
    let (width, height, ink) = parse(&text, glyph);
    let size = symbol_size(width, glyph);
    decode(width, height, &ink, size, invert)
}

/// Split a multi-code rendering into its individual code texts, dropping captions.
fn multi_blocks(text: &str) -> Vec<&str> {
    text.trim_end_matches('\n')
        .split("\n\n")
        .map(|block| block.split_once('\n').expect("caption line").1)
        .collect()
}

/// Decode each code block and concatenate the pieces in order.
fn decode_blocks(text: &str) -> String {
    let mut assembled = String::new();
    for block in multi_blocks(text) {
        let (width, height, ink) = parse(block, "half");
        let size = symbol_size(width, "half");
        assembled.push_str(&decode(width, height, &ink, size, false));
    }
    assembled
}

#[test]
fn split_payload_round_trips_in_order() {
    // Lowercase forces byte mode: 3000 bytes exceed version 40-L (~2953), so the
    // payload is emitted as two codes that concatenate back to the original.
    let payload = "a".repeat(3000);
    let text = render(&["-e", "L", &payload]);
    assert!(
        multi_blocks(&text).len() > 1,
        "expected the payload to split"
    );
    assert_eq!(decode_blocks(&text), payload);
}

#[test]
fn max_size_split_round_trips_in_order() {
    let payload = "a".repeat(3000);
    let text = render(&["-e", "L", "--max-size", "500", &payload]);
    let blocks = multi_blocks(&text);
    assert_eq!(blocks.len(), 6, "3000 bytes at 500 bytes per code");
    assert_eq!(decode_blocks(&text), payload);
}

#[test]
fn chunk_split_round_trips_in_order() {
    let payload = "a".repeat(2500);
    let text = render(&["-e", "L", "--chunk", "5", &payload]);
    let blocks = multi_blocks(&text);
    assert_eq!(blocks.len(), 5, "--chunk 5 forces five codes");
    assert_eq!(decode_blocks(&text), payload);
}

#[test]
fn half_round_trips() {
    for payload in ["hello", "https://example.com/abc", "0123456789"] {
        assert_eq!(round_trip(payload, "half", false), payload);
    }
}

#[test]
fn half_inverted_round_trips() {
    assert_eq!(round_trip("hello", "half", true), "hello");
}

#[test]
fn quadrant_round_trips() {
    assert_eq!(round_trip("hello", "quadrant", false), "hello");
    assert_eq!(round_trip("hello", "quadrant", true), "hello");
}

#[test]
fn braille_round_trips() {
    assert_eq!(round_trip("hello", "braille", false), "hello");
    assert_eq!(round_trip("hello", "braille", true), "hello");
}

#[test]
fn higher_error_correction_round_trips() {
    let text = render(&["-e", "H", "redundancy"]);
    let (width, height, ink) = parse(&text, "half");
    let size = symbol_size(width, "half");
    assert_eq!(decode(width, height, &ink, size, false), "redundancy");
}

#[test]
fn non_ascii_round_trips() {
    // Non-ASCII text is declared with a UTF-8 ECI and must still decode exactly.
    for payload in ["你好，世界", "参数 --file 文件", "한국어テスト😀"] {
        assert_eq!(round_trip(payload, "half", false), payload);
        assert_eq!(round_trip(payload, "quadrant", false), payload);
    }
}

#[test]
fn chunk_split_non_ascii_round_trips_in_order() {
    // A CJK payload forced into several codes: every code must carry its own
    // UTF-8 declaration and concatenate back to the original.
    let payload = "汉字内容测试".repeat(400);
    let text = render(&["-e", "L", "--chunk", "6", &payload]);
    let blocks = multi_blocks(&text);
    assert_eq!(blocks.len(), 6, "--chunk 6 forces six codes");
    assert_eq!(decode_blocks(&text), payload);
}

#[test]
fn chinese_file_split_round_trips_in_order() {
    let raw = std::fs::read("README.zh-CN.md").expect("readme present");
    let mut expected = String::from_utf8(raw).expect("utf-8");
    if expected.ends_with("\r\n") {
        expected.truncate(expected.len() - 2);
    } else if expected.ends_with('\n') {
        expected.pop();
    }
    let text = render(&["-f", "README.zh-CN.md", "-c", "30"]);
    let blocks = multi_blocks(&text);
    assert_eq!(blocks.len(), 30, "expected 30 codes");
    assert_eq!(decode_blocks(&text), expected);
}

#[test]
fn rendered_grid_matches_the_matrix() {
    use qrcode::{Color, QrCode};

    let payload = "hello";
    let qr = QrCode::new(payload.as_bytes()).unwrap();
    let matrix = qr.width();
    let expected: Vec<bool> = qr.to_colors().iter().map(|c| *c == Color::Dark).collect();

    for (glyph, invert) in [
        ("half", false),
        ("quadrant", false),
        ("braille", false),
        ("half", true),
    ] {
        let mut args = vec!["--glyphs", glyph, "-e", "M"];
        if invert {
            args.push("--invert");
        }
        args.push(payload);
        let (width, _height, ink) = parse(&render(&args), glyph);
        assert_eq!(symbol_size(width, glyph), matrix);

        for y in 0..matrix {
            for x in 0..matrix {
                let cell = ink[(y + DEFAULT_BORDER) * width + (x + DEFAULT_BORDER)];
                let got_dark = if invert { cell } else { !cell };
                assert_eq!(
                    got_dark,
                    expected[y * matrix + x],
                    "glyph={glyph} invert={invert} at ({x},{y})"
                );
            }
        }
    }
}

#[test]
fn cjk_round_trips_at_every_error_level() {
    for payload in CJK_SAMPLES {
        for ec in ["L", "M", "Q", "H"] {
            let decoded = round_trip_with(payload, "half", false, &["-e", ec]);
            assert_eq!(&decoded, payload, "ec={ec} payload={payload:?}");
            assert!(
                !decoded.contains('\u{FFFD}'),
                "lossy decode at ec={ec} for {payload:?}"
            );
        }
    }
}

#[test]
fn cjk_round_trips_across_glyph_sets() {
    for glyph in ["half", "quadrant", "braille"] {
        for payload in CJK_SAMPLES {
            let decoded = round_trip_with(payload, glyph, false, &[]);
            assert_eq!(&decoded, payload, "glyph={glyph} payload={payload:?}");
        }
    }
}

#[test]
fn cjk_round_trips_when_inverted() {
    for payload in CJK_SAMPLES {
        let decoded = round_trip_with(payload, "half", true, &[]);
        assert_eq!(&decoded, payload, "inverted payload={payload:?}");
    }
}

#[test]
fn cjk_split_round_trips_in_order_without_symbol_loss() {
    // A CJK payload large enough to split by both --chunk and --max-size. Every
    // code must decode losslessly and, in order, reproduce the whole payload.
    let payload = "中文测试 한국어 日本語".repeat(250);
    for extra in [
        ["-e", "L", "--chunk", "7"],
        ["-e", "L", "--max-size", "400"],
    ] {
        let mut args = extra.to_vec();
        args.push("--");
        args.push(payload.as_str());
        let text = render(&args);
        let blocks = multi_blocks(&text);
        assert!(blocks.len() > 1, "expected a split for {extra:?}");

        let mut assembled = String::new();
        for block in &blocks {
            let (width, height, ink) = parse(block, "half");
            let size = symbol_size(width, "half");
            let part = decode(width, height, &ink, size, false);
            assert!(!part.is_empty(), "empty code for {extra:?}");
            assert!(
                !part.contains('\u{FFFD}'),
                "lossy code for {extra:?}: {part:?}"
            );
            assembled.push_str(&part);
        }
        assert_eq!(assembled, payload, "reassembly mismatch for {extra:?}");
    }
}
