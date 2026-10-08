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
