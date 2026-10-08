//! QR encoding, wrapping the `qrcode` crate (FR-2.*).

use qrcode::types::QrError;
use qrcode::{EcLevel, QrCode};

use crate::error::AppError;
use crate::types::Ec;

fn ec_level(ec: Ec) -> EcLevel {
    match ec {
        Ec::L => EcLevel::L,
        Ec::M => EcLevel::M,
        Ec::Q => EcLevel::Q,
        Ec::H => EcLevel::H,
    }
}

/// Whether `data` fits in a single symbol at `ec`.
fn fits(data: &[u8], ec: Ec) -> bool {
    QrCode::with_error_correction_level(data, ec_level(ec)).is_ok()
}

/// Encode `data` into an ordinary QR code at the exact error level.
///
/// `qrcode` never boosts the error level, matching qrpipe's `boost_error=false`.
pub fn encode(data: &[u8], ec: Ec) -> Result<QrCode, AppError> {
    QrCode::with_error_correction_level(data, ec_level(ec)).map_err(|error| match error {
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
/// single symbol can hold), and the payload is spread across at least
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
/// limit. The fewest symbols that can hold `data` under that cap is a lower
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
    let cap = max_bytes.map_or(capacity, |bytes| bytes.min(capacity));
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
    fn max_size_caps_and_balances_every_chunk() {
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
    fn max_size_keeps_a_fitting_payload_as_one_code() {
        assert_eq!(split_chunks(&[b'a'; 50], Ec::L, Some(1000), None).len(), 1);
    }

    #[test]
    fn max_size_splits_a_small_payload() {
        let payload = b"abcdefghij";
        let chunks = split_chunks(payload, Ec::L, Some(4), None);
        assert_eq!(chunks.len(), 3);
        assert!(chunks.iter().all(|chunk| chunk.len() <= 4));
        assert_eq!(chunks.concat(), payload);
    }

    #[test]
    fn max_size_above_the_symbol_limit_is_clamped() {
        let data = vec![b'a'; 4000];
        let chunks = split_chunks(&data, Ec::L, Some(9000), None);
        assert_eq!(chunks.len(), 2, "clamped cap matches the symbol limit");
        assert!(chunks.iter().all(|chunk| chunk.len() <= 2953));
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn encode_multi_honors_max_size() {
        let codes = encode_multi(&vec![b'a'; 250], Ec::L, Some(100), None).unwrap();
        assert_eq!(codes.len(), 3);
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
    fn chunk_combines_with_max_size() {
        let data = vec![b'a'; 1000];
        let chunks = split_chunks(&data, Ec::L, Some(300), Some(5));
        let sizes: Vec<usize> = chunks.iter().map(|chunk| chunk.len()).collect();
        assert_eq!(sizes.len(), 5);
        assert!(sizes.iter().all(|&size| size <= 300), "{sizes:?}");
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn chunk_one_matches_no_floor() {
        assert_eq!(split_chunks(b"hello", Ec::M, None, Some(1)).len(), 1);
    }
}
