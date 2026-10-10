//! QR encoding, wrapping the `qrcode` crate (FR-2.*).

use qrcode::bits::Bits;
use qrcode::optimize::{Parser, Segment};
use qrcode::types::{Mode, QrError};
use qrcode::{EcLevel, QrCode, Version};

use crate::error::AppError;
use crate::types::Ec;

/// ECI designator for UTF-8 (ISO/IEC 18004 assignment 26).
const UTF8_ECI: u32 = 26;

/// Highest ordinary QR version, used to bound the ECI version search.
const MAX_VERSION: i16 = 40;

fn ec_level(ec: Ec) -> EcLevel {
    match ec {
        Ec::L => EcLevel::L,
        Ec::M => EcLevel::M,
        Ec::Q => EcLevel::Q,
        Ec::H => EcLevel::H,
    }
}

/// Whether `data` is text whose bytes must be declared as UTF-8 to decode
/// reliably.
///
/// A QR symbol stores byte data with no intrinsic charset; a scanner that sees
/// no ECI falls back to a decoder-chosen default (frequently a legacy East-Asian
/// set), so raw UTF-8 text can come back as mojibake. Pure ASCII is unambiguous
/// and non-UTF-8 payloads are genuinely binary, so neither needs a declaration.
fn needs_utf8_eci(data: &[u8]) -> bool {
    std::str::from_utf8(data).is_ok_and(|text| !text.is_ascii())
}

/// Segments for UTF-8 text, with any Kanji-mode segment forced back to bytes.
///
/// The crate's optimizer treats byte pairs in the Shift-JIS Kanji ranges as
/// Kanji, but UTF-8 CJK bytes routinely fall in those ranges. Emitting Kanji
/// segments would make scanners decode them as Shift-JIS (mojibake) and would
/// contradict the UTF-8 ECI, so those ranges are encoded as byte data instead.
/// Numeric and alphanumeric runs are left optimized.
fn utf8_segments(data: &[u8], version: Version) -> Vec<Segment> {
    Parser::new(data)
        .optimize(version)
        .map(|segment| match segment.mode {
            Mode::Kanji => Segment {
                mode: Mode::Byte,
                ..segment
            },
            _ => segment,
        })
        .collect()
}

/// Build a QR code for `data` at the exact error level.
///
/// Non-ASCII text is prefixed with a UTF-8 ECI so scanners decode the bytes as
/// UTF-8. Binary and ASCII payloads are encoded unchanged. The symbol is kept at
/// the smallest version that fits (`qrcode` never boosts the error level).
fn build(data: &[u8], ec: Ec) -> Result<QrCode, QrError> {
    let level = ec_level(ec);
    if !needs_utf8_eci(data) {
        return QrCode::with_error_correction_level(data, level);
    }
    // Seed from the plain minimal version: the ECI's 12 bits rarely push the
    // symbol up a version, so this usually succeeds on the first try.
    let seed = match QrCode::with_error_correction_level(data, level)?.version() {
        Version::Normal(version) | Version::Micro(version) => version,
    };
    for version in seed..=MAX_VERSION {
        let version = Version::Normal(version);
        let mut bits = Bits::new(version);
        if bits.push_eci_designator(UTF8_ECI).is_err() {
            break;
        }
        let segments = utf8_segments(data, version).into_iter();
        if bits.push_segments(data, segments).is_err() || bits.push_terminator(level).is_err() {
            continue;
        }
        if let Ok(qr) = QrCode::with_bits(bits, level) {
            return Ok(qr);
        }
    }
    Err(QrError::DataTooLong)
}

/// Whether `data` fits in a single symbol at `ec`.
fn fits(data: &[u8], ec: Ec) -> bool {
    build(data, ec).is_ok()
}

/// Encode `data` into an ordinary QR code at the exact error level.
///
/// `qrcode` never boosts the error level, matching qrpipe's `boost_error=false`.
pub fn encode(data: &[u8], ec: Ec) -> Result<QrCode, AppError> {
    build(data, ec).map_err(|error| match error {
        QrError::DataTooLong => {
            let max = max_prefix_fitting(data, ec);
            AppError::Input(format!(
                "input too long: {} bytes (maximum {} bytes at error-correction level {ec:?})",
                data.len(),
                max
            ))
        }
        _ => AppError::Input("input cannot be encoded as a QR code".to_string()),
    })
}

/// Encode `data`, splitting it across as many QR codes as needed.
///
/// Each code holds at most `max_bytes` payload bytes (when `None`, the most a
/// single symbol can hold; a value below 1 is treated as 1), and the payload is
/// spread across at least
/// `min_chunks` codes when the content allows (advisory; one code per character
/// is the ceiling). The chunks are as evenly sized as capacity allows, and when
/// `data` is valid UTF-8 the cuts retreat to character boundaries, so a
/// multi-byte character is never divided between two symbols. A payload that is
/// within every limit yields a single code.
pub fn encode_multi(
    data: &[u8],
    ec: Ec,
    max_bytes: Option<usize>,
    min_chunks: Option<usize>,
) -> Result<Vec<QrCode>, AppError> {
    split_chunks(data, ec, max_bytes, min_chunks)
        .into_iter()
        .map(|chunk| encode(chunk, ec))
        .collect()
}

/// Extra symbols [`split_chunks`] may add while seeking an even cut before it
/// falls back to a greedy maximal split.
const BALANCE_SLACK: usize = 8;

/// Split `data` into contiguous chunks that each fit at `ec` and stay within
/// `max_bytes`, sized as evenly as possible, using at least `min_chunks` codes
/// when the content allows.
///
/// The effective byte cap is `max_bytes` clamped to the content's own per-symbol
/// limit and floored at one byte, so a zero or tiny cap yields one byte (or one
/// whole character) per code rather than dividing by zero. The fewest symbols
/// that can hold `data` under that cap is a lower
/// bound; `min_chunks` raises it (advisory, capped at one code per character).
/// An even cut at that count (or up to [`BALANCE_SLACK`] more, when a
/// content-dependent mode makes an even cut unencodable) is preferred. A greedy
/// maximal split is the guaranteed fallback.
fn split_chunks(
    data: &[u8],
    ec: Ec,
    max_bytes: Option<usize>,
    min_chunks: Option<usize>,
) -> Vec<&[u8]> {
    let capacity = max_prefix_fitting(data, ec).max(1);
    // Floor the cap at one byte so a zero (or absent) `max_bytes` cannot reach
    // `div_ceil(0)` below; a zero cap splits one byte per code instead of panicking.
    let cap = max_bytes
        .map_or(capacity, |bytes| bytes.min(capacity))
        .max(1);
    let text = std::str::from_utf8(data).ok();
    // A code must hold at least one character (or byte), so this bounds the count.
    let most = text.map_or(data.len(), |text| text.chars().count()).max(1);
    let floor = min_chunks.unwrap_or(1).min(most);
    let fewest = data.len().div_ceil(cap);
    let first = fewest.max(floor);
    if first <= 1 && fits(data, ec) {
        return vec![data];
    }
    for count in first..=first + BALANCE_SLACK {
        let chunks = even_chunks(data, count, text);
        if chunks
            .iter()
            .all(|chunk| !chunk.is_empty() && chunk.len() <= cap && fits(chunk, ec))
        {
            return chunks;
        }
    }
    greedy_chunks(data, ec, text, cap)
}

/// Split `data` into `count` chunks whose byte lengths differ by at most one,
/// snapping each cut back to a UTF-8 character boundary when `data` is text.
fn even_chunks<'a>(data: &'a [u8], count: usize, text: Option<&str>) -> Vec<&'a [u8]> {
    let len = data.len();
    let mut chunks = Vec::with_capacity(count);
    let mut start = 0;
    for index in 1..=count {
        let mut end = len * index / count;
        if let Some(text) = text {
            while end > start && !text.is_char_boundary(end) {
                end -= 1;
            }
        }
        let end = end.max(start);
        chunks.push(&data[start..end]);
        start = end;
    }
    chunks
}

/// Split `data` greedily: each chunk is the largest prefix of the remainder that
/// fits at `ec` and stays within `cap`. Feasible whenever a split is needed.
fn greedy_chunks<'a>(data: &'a [u8], ec: Ec, text: Option<&str>, cap: usize) -> Vec<&'a [u8]> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let mut end = offset + max_prefix_fitting(&data[offset..], ec).min(cap);
        if let Some(text) = text {
            while end > offset && !text.is_char_boundary(end) {
                end -= 1;
            }
        }
        if end == offset {
            // The cap is smaller than the next character; take one whole
            // character to guarantee progress without splitting it.
            end = offset + 1;
            if let Some(text) = text {
                while end < data.len() && !text.is_char_boundary(end) {
                    end += 1;
                }
            }
        }
        chunks.push(&data[offset..end]);
        offset = end;
    }
    chunks
}

/// The largest length any single QR symbol can hold (numeric mode, version 40-L).
const ABSOLUTE_MAX_BYTES: usize = 7089;

/// Largest prefix of `data`, in bytes, that still fits at `ec`.
///
/// Bounded by [`ABSOLUTE_MAX_BYTES`] so the search cost does not grow with an
/// arbitrarily large rejected input.
fn max_prefix_fitting(data: &[u8], ec: Ec) -> usize {
    let (mut lo, mut hi) = (0usize, data.len().min(ABSOLUTE_MAX_BYTES));
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits(&data[..mid], ec) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_with_exact_error_level() {
        for ec in [Ec::L, Ec::M, Ec::Q, Ec::H] {
            let qr = encode(b"hello", ec).unwrap();
            assert_eq!(qr.error_correction_level(), ec_level(ec));
        }
    }

    #[test]
    fn oversized_input_is_an_input_error() {
        let huge = vec![b'x'; 5000];
        assert!(matches!(encode(&huge, Ec::L), Err(AppError::Input(_))));
    }

    #[test]
    fn too_long_error_reports_both_lengths() {
        let data = vec![b'x'; 4000];
        let Err(AppError::Input(message)) = encode(&data, Ec::L) else {
            panic!("expected an input error");
        };
        assert!(message.contains("4000"), "{message}");
        assert!(message.contains("2953"), "{message}");
        assert!(message.contains('L'), "{message}");
    }

    #[test]
    fn byte_limit_at_level_l_is_2953() {
        assert!(encode(&vec![b'x'; 2953], Ec::L).is_ok());
        assert!(encode(&vec![b'x'; 2954], Ec::L).is_err());
    }

    #[test]
    fn multi_keeps_a_fitting_payload_as_one_code() {
        assert_eq!(encode_multi(b"hello", Ec::M, None, None).unwrap().len(), 1);
    }

    #[test]
    fn multi_splits_an_oversized_payload() {
        let codes = encode_multi(&vec![b'x'; 4000], Ec::L, None, None).unwrap();
        assert!(codes.len() > 1, "expected more than one code");
    }

    #[test]
    fn multi_splits_the_payload_evenly() {
        let data = vec![b'a'; 3000];
        let chunks = split_chunks(&data, Ec::L, None, None);
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        assert_eq!(sizes, vec![1500, 1500]);
    }

    #[test]
    fn multi_splits_evenly_across_more_than_two_codes() {
        let data = vec![b'a'; 6000];
        let chunks = split_chunks(&data, Ec::L, None, None);
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        assert_eq!(sizes, vec![2000, 2000, 2000]);
    }

    #[test]
    fn split_chunks_cover_the_payload_in_order() {
        let data: Vec<u8> = (0..4000u32).map(|i| b'a' + (i % 26) as u8).collect();
        let chunks = split_chunks(&data, Ec::M, None, None);
        assert!(chunks.len() > 1, "expected a split");
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn multi_even_split_respects_character_boundaries() {
        // 1000 three-byte characters (3000 bytes); no cut may split one.
        let text = "\u{4f60}".repeat(1000);
        let chunks = split_chunks(text.as_bytes(), Ec::L, None, None);
        for chunk in &chunks {
            assert!(
                std::str::from_utf8(chunk).is_ok(),
                "chunk split a character"
            );
        }
        assert_eq!(chunks.concat(), text.as_bytes());
    }

    #[test]
    fn multi_balances_by_trading_a_symbol_when_modes_differ() {
        // 14000 digits encode densely (numeric mode), but a trailing letter forces
        // byte mode, so a strictly even split needs more symbols than the dense
        // minimum of two.
        let mut data = vec![b'1'; 14000];
        data.extend_from_slice(&[b'a'; 100]);
        let chunks = split_chunks(&data, Ec::L, None, None);
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        let spread = *sizes.iter().max().unwrap() - *sizes.iter().min().unwrap();
        assert!(
            sizes.len() >= 3,
            "expected extra symbols for balance: {sizes:?}"
        );
        assert!(spread <= 1, "unbalanced: {sizes:?}");
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn max_bytes_caps_and_balances_every_chunk() {
        let data = vec![b'a'; 250];
        let chunks = split_chunks(&data, Ec::L, Some(100), None);
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        assert_eq!(sizes.len(), 3);
        assert!(sizes.iter().all(|&size| size <= 100), "{sizes:?}");
        assert_eq!(
            *sizes.iter().max().unwrap() - *sizes.iter().min().unwrap(),
            1,
            "{sizes:?}"
        );
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn max_bytes_keeps_a_fitting_payload_as_one_code() {
        assert_eq!(split_chunks(&[b'a'; 50], Ec::L, Some(1000), None).len(), 1);
    }

    #[test]
    fn max_bytes_splits_a_small_payload() {
        let payload = b"abcdefghij";
        let chunks = split_chunks(payload, Ec::L, Some(4), None);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|chunk| chunk.len() <= 4));
        assert_eq!(chunks.concat(), payload);
    }

    #[test]
    fn max_bytes_above_the_symbol_limit_is_clamped() {
        let data = vec![b'a'; 4000];
        let chunks = split_chunks(&data, Ec::L, Some(9000), None);
        assert_eq!(chunks.len(), 2, "clamped cap matches the symbol limit");
        assert!(chunks.iter().all(|chunk| chunk.len() <= 2953));
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn encode_multi_honors_max_bytes() {
        let codes = encode_multi(&vec![b'a'; 250], Ec::L, Some(100), None).unwrap();
        assert_eq!(codes.len(), 3);
    }

    #[test]
    fn zero_max_bytes_is_floored_instead_of_panicking() {
        // Regression: a zero cap used to reach `div_ceil(0)` and panic. It is
        // floored to one byte, so the payload still splits (one byte, or one
        // whole character, per code) and no character is divided.
        let ascii = encode_multi(b"abc", Ec::L, Some(0), None).unwrap();
        assert_eq!(ascii.len(), 3, "one byte per code");

        let cjk = encode_multi("汉字".as_bytes(), Ec::L, Some(0), None).unwrap();
        assert_eq!(cjk.len(), 2, "one whole character per code");
    }

    #[test]
    fn chunk_raises_the_count_and_balances() {
        let data = vec![b'a'; 100];
        let chunks = split_chunks(&data, Ec::L, None, Some(4));
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        assert_eq!(sizes, vec![25, 25, 25, 25]);
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn chunk_below_the_natural_count_is_ignored() {
        // 4000 bytes need two codes regardless; a smaller floor does not reduce it.
        let data = vec![b'a'; 4000];
        assert_eq!(split_chunks(&data, Ec::L, None, Some(2)).len(), 2);
    }

    #[test]
    fn chunk_larger_than_the_content_is_best_effort() {
        // Two bytes can become at most two codes.
        assert_eq!(split_chunks(b"hi", Ec::L, None, Some(10)).len(), 2);
    }

    #[test]
    fn chunk_combines_with_max_bytes() {
        let data = vec![b'a'; 1000];
        let chunks = split_chunks(&data, Ec::L, Some(300), Some(5));
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        assert_eq!(sizes.len(), 5);
        assert!(sizes.iter().all(|&size| size <= 300), "{sizes:?}");
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn utf8_text_is_never_encoded_as_kanji() {
        // A sample that the crate's optimizer would split into Kanji segments.
        let text = "反相),\n`render` 再按字形集把网格打包成字符。";
        let version = Version::Normal(10);

        let raw: Vec<_> = Parser::new(text.as_bytes()).optimize(version).collect();
        assert!(
            raw.iter().any(|segment| segment.mode == Mode::Kanji),
            "expected the crate to (mis)detect kanji in UTF-8 text"
        );

        let segments = utf8_segments(text.as_bytes(), version);
        assert!(
            segments.iter().all(|segment| segment.mode != Mode::Kanji),
            "kanji segments must be downgraded to bytes"
        );
        assert_eq!(segments.first().unwrap().begin, 0);
        assert_eq!(segments.last().unwrap().end, text.len());
        for pair in segments.windows(2) {
            assert_eq!(pair[0].end, pair[1].begin, "segments must be contiguous");
        }
    }

    /// CJK and mixed-script payloads; entry 5 is the byte pattern that was
    /// previously mis-encoded as Shift-JIS Kanji.
    const CJK_SAMPLES: &[&str] = &[
        "简体中文：你好，世界！",
        "繁體中文：你好，世界！",
        "ひらがな カタカナ 漢字 の テスト",
        "한국어 테스트 안녕하세요",
        "反相）,再按字形集把网格打包成字符。",
        "混合 mixed ABC 123 한자 漢字 😀 ，。！？「」《》；：",
        "参数 file 文件 值 = 42",
    ];

    #[test]
    fn cjk_samples_need_a_utf8_eci() {
        for sample in CJK_SAMPLES {
            assert!(needs_utf8_eci(sample.as_bytes()), "{sample:?}");
        }
    }

    #[test]
    fn cjk_samples_are_never_split_into_kanji_segments() {
        // The invariant that keeps scanners from reading CJK as Shift-JIS.
        for sample in CJK_SAMPLES {
            let bytes = sample.as_bytes();
            for version in [Version::Normal(1), Version::Normal(10), Version::Normal(40)] {
                let segments = utf8_segments(bytes, version);
                assert!(
                    segments.iter().all(|segment| segment.mode != Mode::Kanji),
                    "kanji segment for {sample:?} at version {version:?}"
                );
                // Segments must tile the payload exactly: contiguous, gapless,
                // and covering every byte once.
                assert_eq!(segments.first().unwrap().begin, 0, "{sample:?}");
                assert_eq!(segments.last().unwrap().end, bytes.len(), "{sample:?}");
                for pair in segments.windows(2) {
                    assert_eq!(pair[0].end, pair[1].begin, "{sample:?}");
                }
            }
        }
    }

    #[test]
    fn cjk_samples_encode_and_fit() {
        // Every CJK sample encodes at each level and reports a matching `fits`.
        for sample in CJK_SAMPLES {
            for ec in [Ec::L, Ec::M, Ec::Q, Ec::H] {
                assert!(fits(sample.as_bytes(), ec), "{sample:?} at {ec:?}");
                assert!(
                    encode(sample.as_bytes(), ec).is_ok(),
                    "{sample:?} at {ec:?}"
                );
            }
        }
    }

    #[test]
    fn cjk_encoding_differs_from_the_plain_encoder() {
        // Confirms the UTF-8 ECI / byte-mode path is actually taken for CJK: the
        // symbol must not equal a plain (Kanji-prone) encoding of the same bytes.
        for sample in CJK_SAMPLES {
            let ours = encode(sample.as_bytes(), Ec::M).unwrap();
            let plain =
                QrCode::with_error_correction_level(sample.as_bytes(), ec_level(Ec::M)).unwrap();
            assert_ne!(ours.to_colors(), plain.to_colors(), "{sample:?}");
        }
    }

    #[test]
    fn chunk_one_matches_no_floor() {
        assert_eq!(split_chunks(b"hello", Ec::M, None, Some(1)).len(), 1);
    }

    #[test]
    fn chunk_split_never_divides_a_character() {
        // Mixed 1-, 2-, 3- and 4-byte characters; whatever the forced count, no
        // cut may fall inside a character and the chunks must reassemble.
        let text = "汉aé😀字\n".repeat(300);
        for count in [2, 3, 7, 30, 97] {
            let chunks = split_chunks(text.as_bytes(), Ec::L, None, Some(count));
            assert_eq!(chunks.concat(), text.as_bytes(), "count {count}");
            for chunk in &chunks {
                assert!(
                    std::str::from_utf8(chunk).is_ok(),
                    "split a character at count {count}"
                );
            }
        }
    }

    #[test]
    fn needs_utf8_eci_only_for_non_ascii_text() {
        assert!(!needs_utf8_eci(b"hello"), "ascii needs no declaration");
        assert!(needs_utf8_eci("汉字".as_bytes()), "cjk text needs one");
        assert!(needs_utf8_eci("café".as_bytes()), "latin-1 range needs one");
        assert!(!needs_utf8_eci(&[0xff, 0x00, 0x80]), "binary needs none");
    }

    #[test]
    fn ascii_encoding_matches_the_plain_encoder() {
        // No ECI is inserted, so the symbol is byte-for-byte the crate's own.
        let data = b"plain ascii payload";
        let ours = encode(data, Ec::M).unwrap();
        let plain = QrCode::with_error_correction_level(data, ec_level(Ec::M)).unwrap();
        assert_eq!(ours.to_colors(), plain.to_colors());
    }

    #[test]
    fn non_ascii_encoding_is_declared_as_utf8() {
        // The UTF-8 ECI changes the bit stream, so the matrix differs from a
        // plain encoding of the same bytes.
        let data = "参数".as_bytes();
        let ours = encode(data, Ec::M).unwrap();
        let plain = QrCode::with_error_correction_level(data, ec_level(Ec::M)).unwrap();
        assert_ne!(
            ours.to_colors(),
            plain.to_colors(),
            "expected an ECI-declared symbol"
        );
    }

    #[test]
    fn non_ascii_uses_the_smallest_version_that_fits() {
        let data = "中文内容".as_bytes();
        let ours = encode(data, Ec::L).unwrap();
        let plain = QrCode::with_error_correction_level(data, ec_level(Ec::L)).unwrap();
        // The ECI adds 12 bits, so the version can be equal or one larger.
        assert!(
            ours.width() >= plain.width() && ours.width() <= plain.width() + 4,
            "unexpected version jump: {} vs {}",
            ours.width(),
            plain.width()
        );
    }
}
