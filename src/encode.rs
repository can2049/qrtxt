//! Symbol encoding, wrapping a barcode crate per symbology (FR-2.*).
//!
//! QR codes come from the `qrcode` crate and Data Matrix (ECC 200) codes from
//! the `datamatrix` crate; both are converted to the neutral [`Symbol`] grid so
//! nothing downstream depends on either library.

use datamatrix::{DataMatrix, SymbolList};
use qrcode::bits::Bits;
use qrcode::optimize::{Parser, Segment};
use qrcode::types::{Mode, QrError};
use qrcode::{Color, EcLevel, QrCode, Version};
use rxing::common::CharacterSet;
use rxing::pdf417::encoder::PDF417;

use crate::error::AppError;
use crate::types::{Ec, EncodeOptions, Pdf417Ec, Symbol};

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

/// Convert a built QR code into the neutral [`Symbol`] grid (FR-2.4).
///
/// The rest of the crate (rendering in particular) consumes only `Symbol`, so it
/// stays independent of the `qrcode` crate.
fn to_symbol(qr: &QrCode) -> Symbol {
    let width = qr.width();
    let dark = qr
        .to_colors()
        .iter()
        .map(|color| matches!(color, Color::Dark))
        .collect();
    Symbol::new(width, width, dark)
}

/// A barcode symbology: bytes in, one [`Symbol`] out, plus its own per-symbol
/// capacity. The splitter in [`encode_multi`] is written once against this.
trait Codec {
    /// Encode `data` into a single symbol, or explain why it does not fit.
    fn encode(&self, data: &[u8]) -> Result<Symbol, AppError>;
    /// Whether `data` fits a single symbol.
    fn fits(&self, data: &[u8]) -> bool;
    /// The largest byte length a single symbol can hold; bounds the split search.
    fn capacity(&self) -> usize;
}

/// Largest byte length any single QR symbol can hold (version 40-L numeric): the
/// bound for the split search (FR-2.5).
const QR_MAX_BYTES: usize = 7089;

/// Largest byte length any single Data Matrix square symbol can hold (144x144
/// numeric): the bound for the split search (FR-2.6).
const DATAMATRIX_MAX_BYTES: usize = 3116;

/// Upper bound on the bytes a single PDF417 symbol can hold: the bound for the
/// split search.
///
/// PDF417's densest mode is numeric compaction (about 2.9 bytes per data
/// codeword), and the most data codewords any symbol carries is 925 (level 0),
/// so no payload can exceed roughly 2710 bytes. This rounds that up: being an
/// over-estimate only costs a few extra probes in [`max_prefix_fitting`].
const PDF417_MAX_BYTES: usize = 2800;

/// QR codec: the `qrcode` crate at the exact error level (`qrcode` never boosts
/// it, matching qrpipe's `boost_error=false`).
struct QrCodec {
    ec: Ec,
}

impl Codec for QrCodec {
    fn encode(&self, data: &[u8]) -> Result<Symbol, AppError> {
        build(data, self.ec)
            .map(|qr| to_symbol(&qr))
            .map_err(|error| match error {
                QrError::DataTooLong => {
                    let max = max_prefix_fitting(self, data);
                    AppError::Input(format!(
                        "input too long: {} bytes (maximum {} bytes at error-correction level {:?})",
                        data.len(),
                        max,
                        self.ec
                    ))
                }
                _ => AppError::Input("input cannot be encoded as a QR code".to_string()),
            })
    }

    fn fits(&self, data: &[u8]) -> bool {
        fits(data, self.ec)
    }

    fn capacity(&self) -> usize {
        QR_MAX_BYTES
    }
}

/// Data Matrix codec (ECC 200), restricted to square symbols.
///
/// Data Matrix has no user-selectable error-correction level — it is fixed by the
/// symbol size — so [`Codec::capacity`] uses the largest square symbol and the
/// error-correction level is not consulted here (FR-2.6).
struct DataMatrixCodec;

impl Codec for DataMatrixCodec {
    fn encode(&self, data: &[u8]) -> Result<Symbol, AppError> {
        data_matrix_symbol(data).map_err(|_| {
            let max = max_prefix_fitting(self, data);
            AppError::Input(format!(
                "input too long: {} bytes (maximum {} bytes in a Data Matrix symbol)",
                data.len(),
                max
            ))
        })
    }

    fn fits(&self, data: &[u8]) -> bool {
        data_matrix_symbol(data).is_ok()
    }

    fn capacity(&self) -> usize {
        DATAMATRIX_MAX_BYTES
    }
}

/// Encode `data` into a square Data Matrix symbol as a neutral [`Symbol`].
///
/// Valid UTF-8 goes through the crate's string path, which declares a UTF-8 ECI
/// for text outside Latin-1 and uses Latin-1 otherwise (the Data Matrix default
/// charset); any other bytes use the raw path. Only square symbols are requested
/// so terminal output stays square.
fn data_matrix_symbol(data: &[u8]) -> Result<Symbol, datamatrix::data::DataEncodingError> {
    let symbols = SymbolList::default().enforce_square();
    let code = match std::str::from_utf8(data) {
        Ok(text) => DataMatrix::encode_str(text, symbols)?,
        Err(_) => DataMatrix::encode(data, symbols)?,
    };
    let bitmap = code.bitmap();
    let (width, height) = (bitmap.width(), bitmap.height());
    let mut dark = vec![false; width * height];
    for (x, y) in bitmap.pixels() {
        dark[y * width + x] = true;
    }
    Ok(Symbol::new(width, height, dark))
}

/// PDF417 codec: the `rxing` crate's low-level encoder.
///
/// Unlike QR and Data Matrix, the PDF417 encoder takes *text*, not bytes, so the
/// payload is mapped through Latin-1 when it is not valid UTF-8 and declared as
/// UTF-8 when it is non-ASCII text (see [`pdf417_symbol`]); both paths round-trip
/// the original bytes. The error-correction level is a number from 0 to 8, and
/// `columns` optionally pins the data-column count to shape the symbol.
struct Pdf417Codec {
    ec: Pdf417Ec,
    columns: Option<u32>,
}

impl Codec for Pdf417Codec {
    fn encode(&self, data: &[u8]) -> Result<Symbol, AppError> {
        pdf417_symbol(data, self.ec, self.columns).map_err(|()| {
            let max = max_prefix_fitting(self, data);
            AppError::Input(format!(
                "input too long: {} bytes (maximum {} bytes in a PDF417 symbol at error-correction level {})",
                data.len(),
                max,
                self.ec.get()
            ))
        })
    }

    fn fits(&self, data: &[u8]) -> bool {
        pdf417_symbol(data, self.ec, self.columns).is_ok()
    }

    fn capacity(&self) -> usize {
        PDF417_MAX_BYTES
    }
}

/// PDF417's row bounds (ISO/IEC 15438): at least 3 rows, at most 90.
const PDF417_MIN_ROWS: u32 = 3;
const PDF417_MAX_ROWS: u32 = 90;

/// The largest row count safe for `columns` columns at error level `ec`.
///
/// A PDF417 symbol carries its data-codeword count in a single codeword, so a
/// symbol can address at most 928 data codewords. `rxing` derives the row count
/// from the payload but guards only the *high-level* codeword count, so a large
/// forced column count can make `columns * rows - ecCodewords` exceed 928 and it
/// then indexes its 929-entry codeword table out of bounds — a panic, not an
/// error. Bounding the rows to `(928 + ecCodewords) / columns` keeps the
/// descriptor in range `(FR-2.6)`, so an oversized payload fails with a clean
/// "cannot fit" error instead.
fn pdf417_max_rows(columns: u32, ec: Pdf417Ec) -> u32 {
    // Error-correction codewords double with each level: 2^(level + 1), so 2 at
    // level 0 and 512 at level 8 (ISO/IEC 15438).
    let ec_codewords = 1u32 << (u32::from(ec.get()) + 1);
    ((928 + ec_codewords) / columns).clamp(PDF417_MIN_ROWS, PDF417_MAX_ROWS)
}

/// Encode `data` into a single PDF417 symbol as a neutral [`Symbol`].
///
/// The `rxing` encoder accepts a string rather than raw bytes, so a payload that
/// is not valid UTF-8 is mapped through Latin-1 (byte value to code point
/// `U+0000..U+00FF`) and encoded in the default ISO-8859-1 charset, which returns
/// the exact bytes on decode. Valid UTF-8 text outside ASCII is instead declared
/// with a UTF-8 ECI, so scanners do not fall back to a legacy charset — the same
/// reasoning as the QR encoder. `columns` pins the data-column count when set
/// (the symbol is then `17 * columns + 69` modules wide and grows taller); `None`
/// lets the encoder choose a balanced shape. Fails only for an empty payload or
/// one too large for a single symbol.
fn pdf417_symbol(data: &[u8], ec: Pdf417Ec, columns: Option<u32>) -> Result<Symbol, ()> {
    let (text, utf8_eci): (std::borrow::Cow<'_, str>, bool) = match std::str::from_utf8(data) {
        Ok(text) if !text.is_ascii() => (std::borrow::Cow::Borrowed(text), true),
        Ok(text) => (std::borrow::Cow::Borrowed(text), false),
        Err(_) => (
            data.iter()
                .map(|&byte| byte as char)
                .collect::<String>()
                .into(),
            false,
        ),
    };
    let mut encoder = PDF417::new();
    if utf8_eci {
        encoder.setEncoding(Some(CharacterSet::UTF8));
    }
    if let Some(columns) = columns {
        encoder.setDimensions(
            columns,
            columns,
            pdf417_max_rows(columns, ec),
            PDF417_MIN_ROWS,
        );
    }
    encoder
        .generateBarcodeLogicWithAutoECI(&text, u32::from(ec.get()), utf8_eci)
        .map_err(|_| ())?;
    let matrix = encoder.getBarcodeMatrix().as_ref().ok_or(())?.getMatrix();
    Ok(matrix_to_symbol(&matrix))
}

/// Convert `rxing`'s PDF417 bit matrix into the neutral [`Symbol`] grid.
///
/// The matrix is row-major with `1` marking a bar, but the crate stores it
/// bottom-up, so the rows are reversed to hand the renderers an upright symbol.
fn matrix_to_symbol(matrix: &[Vec<u8>]) -> Symbol {
    let height = matrix.len();
    let width = matrix.first().map_or(0, Vec::len);
    let mut dark = vec![false; width * height];
    for (y, row) in matrix.iter().rev().enumerate() {
        for (x, &value) in row.iter().enumerate() {
            dark[y * width + x] = value == 1;
        }
    }
    Symbol::new(width, height, dark)
}

/// Encode `data` into an ordinary QR code at the exact error level.
///
/// `qrcode` never boosts the error level, matching qrpipe's `boost_error=false`.
pub fn encode(data: &[u8], ec: Ec) -> Result<Symbol, AppError> {
    QrCodec { ec }.encode(data)
}

/// Encode `data` in the symbology selected by `encode`, splitting it across as
/// many symbols as needed.
///
/// Each symbol holds at most `max_bytes` payload bytes (when `None`, the most a
/// single symbol can hold; a value below 1 is treated as 1), and the payload is
/// spread across at least `min_chunks` symbols when the content allows (advisory;
/// one symbol per character is the ceiling). The chunks are as evenly sized as the
/// symbology's capacity allows, and when `data` is valid UTF-8 the cuts retreat to
/// character boundaries, so a multi-byte character is never divided between two
/// symbols. A payload that is within every limit yields a single symbol.
pub fn encode_multi(
    data: &[u8],
    encode: &EncodeOptions,
    max_bytes: Option<usize>,
    min_chunks: Option<usize>,
) -> Result<Vec<Symbol>, AppError> {
    let qr;
    let datamatrix = DataMatrixCodec;
    let pdf417;
    let codec: &dyn Codec = match encode {
        EncodeOptions::Qr { ec } => {
            qr = QrCodec { ec: *ec };
            &qr
        }
        EncodeOptions::DataMatrix => &datamatrix,
        EncodeOptions::Pdf417 { ec, columns } => {
            pdf417 = Pdf417Codec {
                ec: *ec,
                columns: *columns,
            };
            &pdf417
        }
    };
    split_chunks_with(codec, data, max_bytes, min_chunks)
        .into_iter()
        .map(|chunk| codec.encode(chunk))
        .collect()
}

/// Extra symbols the splitter may add while seeking an even cut before it falls
/// back to a greedy maximal split.
const BALANCE_SLACK: usize = 8;

/// Split `data` into contiguous chunks that each fit `codec` and stay within
/// `max_bytes`, sized as evenly as possible, using at least `min_chunks` codes
/// when the content allows.
///
/// The effective byte cap is `max_bytes` clamped to the symbology's own per-symbol
/// limit and floored at one byte, so a zero or tiny cap yields one byte (or one
/// whole character) per code rather than dividing by zero. The fewest symbols
/// that can hold `data` under that cap is a lower
/// bound; `min_chunks` raises it (advisory, capped at one code per character).
/// An even cut at that count (or up to [`BALANCE_SLACK`] more, when a
/// content-dependent mode makes an even cut unencodable) is preferred. A greedy
/// maximal split is the guaranteed fallback.
fn split_chunks_with<'a>(
    codec: &dyn Codec,
    data: &'a [u8],
    max_bytes: Option<usize>,
    min_chunks: Option<usize>,
) -> Vec<&'a [u8]> {
    let capacity = max_prefix_fitting(codec, data).max(1);
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
    if first <= 1 && codec.fits(data) {
        return vec![data];
    }
    for count in first..=first + BALANCE_SLACK {
        let chunks = even_chunks(data, count, text);
        if chunks
            .iter()
            .all(|chunk| !chunk.is_empty() && chunk.len() <= cap && codec.fits(chunk))
        {
            return chunks;
        }
    }
    greedy_chunks(codec, data, text, cap)
}

/// Split a QR payload; a test-only shim over [`split_chunks_with`].
#[cfg(test)]
fn split_chunks(
    data: &[u8],
    ec: Ec,
    max_bytes: Option<usize>,
    min_chunks: Option<usize>,
) -> Vec<&[u8]> {
    split_chunks_with(&QrCodec { ec }, data, max_bytes, min_chunks)
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
/// fits `codec` and stays within `cap`. Feasible whenever a split is needed.
fn greedy_chunks<'a>(
    codec: &dyn Codec,
    data: &'a [u8],
    text: Option<&str>,
    cap: usize,
) -> Vec<&'a [u8]> {
    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let mut end = offset + max_prefix_fitting(codec, &data[offset..]).min(cap);
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

/// Largest prefix of `data`, in bytes, that still fits a single `codec` symbol.
///
/// Bounded by `codec.capacity()` so the search cost does not grow with an
/// arbitrarily large rejected input.
fn max_prefix_fitting(codec: &dyn Codec, data: &[u8]) -> usize {
    let (mut lo, mut hi) = (0usize, data.len().min(codec.capacity()));
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if codec.fits(&data[..mid]) {
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

    /// QR encode options at error level `ec`, for the tests.
    fn qr(ec: Ec) -> EncodeOptions {
        EncodeOptions::Qr { ec }
    }

    #[test]
    fn encodes_with_exact_error_level() {
        for ec in [Ec::L, Ec::M, Ec::Q, Ec::H] {
            let qr = build(b"hello", ec).unwrap();
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
        assert_eq!(
            encode_multi(b"hello", &qr(Ec::M), None, None)
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn datamatrix_encodes_a_square_symbol() {
        let symbols = encode_multi(b"hello", &EncodeOptions::DataMatrix, None, None).unwrap();
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].width(), symbols[0].height(), "must be square");
    }

    #[test]
    fn datamatrix_encodes_non_ascii() {
        // Valid UTF-8 outside Latin-1 is declared with a UTF-8 ECI by the crate.
        assert!(
            encode_multi(
                "你好，世界".as_bytes(),
                &EncodeOptions::DataMatrix,
                None,
                None
            )
            .is_ok()
        );
    }

    #[test]
    fn datamatrix_splits_an_oversized_payload() {
        let codes =
            encode_multi(&vec![b'a'; 4000], &EncodeOptions::DataMatrix, None, None).unwrap();
        assert!(codes.len() > 1, "expected more than one symbol");
    }

    #[test]
    fn datamatrix_split_covers_the_payload() {
        let data = vec![b'a'; 4000];
        let chunks = split_chunks_with(&DataMatrixCodec, &data, None, None);
        assert!(chunks.len() > 1, "expected a split");
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn datamatrix_reports_the_ceiling_for_an_oversized_input() {
        let huge = vec![b'a'; 4000];
        let Err(AppError::Input(message)) = DataMatrixCodec.encode(&huge) else {
            panic!("expected an input error");
        };
        assert!(message.contains("Data Matrix"), "{message}");
        assert!(message.contains("4000"), "{message}");
    }

    /// PDF417 options at error-correction level `ec`, for the tests.
    fn pdf417(ec: u8) -> EncodeOptions {
        EncodeOptions::Pdf417 {
            ec: Pdf417Ec(ec),
            columns: None,
        }
    }

    /// A PDF417 codec with an optional forced column count, for the tests.
    fn pdf417_codec(ec: u8, columns: Option<u32>) -> Pdf417Codec {
        Pdf417Codec {
            ec: Pdf417Ec(ec),
            columns,
        }
    }

    #[test]
    fn pdf417_encodes_a_rectangular_symbol() {
        let symbols = encode_multi(b"hello", &pdf417(2), None, None).unwrap();
        assert_eq!(symbols.len(), 1);
        assert!(symbols[0].width() > 0 && symbols[0].height() > 0);
        assert_ne!(
            symbols[0].width(),
            symbols[0].height(),
            "PDF417 is not square"
        );
    }

    #[test]
    fn pdf417_columns_pin_the_symbol_width() {
        // A symbol is `17 * columns + 69` modules wide; fewer columns is narrower
        // and taller, more columns wider and shorter.
        for columns in [1u32, 2, 5, 12] {
            let symbol = pdf417_codec(2, Some(columns))
                .encode(b"a payload that spans several columns")
                .unwrap();
            assert_eq!(
                symbol.width(),
                17 * columns as usize + 69,
                "columns={columns}"
            );
        }
    }

    #[test]
    fn pdf417_fewer_columns_make_a_taller_symbol() {
        let data = b"a payload that spans several columns";
        let wide = pdf417_codec(2, Some(12)).encode(data).unwrap();
        let narrow = pdf417_codec(2, Some(2)).encode(data).unwrap();
        assert!(
            narrow.height() > wide.height(),
            "narrow {} vs wide {}",
            narrow.height(),
            wide.height()
        );
    }

    #[test]
    fn pdf417_columns_too_few_for_the_payload_fail() {
        // A single column holds at most 90 codewords, so a payload that would fit
        // the default shape no longer fits and is reported as an input error.
        let data = vec![b'a'; 1000];
        assert!(pdf417_codec(2, None).fits(&data), "fits the default shape");
        assert!(
            !pdf417_codec(2, Some(1)).fits(&data),
            "one column is too narrow"
        );
    }

    #[test]
    fn pdf417_max_rows_keeps_columns_times_rows_within_the_codeword_table() {
        // The symbol's data-codeword count is `columns * rows - ecCodewords`; the
        // symbol length descriptor it must fit is bounded by `928 + ecCodewords`
        // total codewords, or rxing indexes its 929-entry table out of bounds.
        for level in 0..=8u8 {
            let ec_codewords = 1u32 << (u32::from(level) + 1);
            for columns in 1..=30u32 {
                let rows = pdf417_max_rows(columns, Pdf417Ec(level));
                assert!(
                    (PDF417_MIN_ROWS..=PDF417_MAX_ROWS).contains(&rows),
                    "level={level} columns={columns}: rows {rows} out of range"
                );
                assert!(
                    columns * rows <= 928 + ec_codewords,
                    "level={level} columns={columns}: {} codewords overflow",
                    columns * rows
                );
            }
        }
    }

    #[test]
    fn pdf417_forced_columns_do_not_panic_on_a_payload_that_overran_the_table() {
        // Regression: 1830 bytes at 20 columns made rxing index its codeword table
        // out of bounds (index 932 into a 929-entry table) and panic. It must now
        // be rejected cleanly, at the single-symbol level and through the split
        // search (`encode`) that probes this size.
        let data = vec![b'a'; 1830];
        let codec = pdf417_codec(2, Some(20));
        assert!(!codec.fits(&data), "20 columns cannot hold this payload");
        assert!(matches!(codec.encode(&data), Err(AppError::Input(_))));

        let codes = encode_multi(
            &data,
            &EncodeOptions::Pdf417 {
                ec: Pdf417Ec(2),
                columns: Some(20),
            },
            None,
            None,
        )
        .expect("splits instead of panicking");
        assert!(
            codes.iter().all(|code| code.width() == 17 * 20 + 69),
            "every code must use the forced columns"
        );
    }

    #[test]
    fn pdf417_forced_columns_never_panic_across_shapes() {
        // Every column count must encode or fail cleanly across a range of sizes;
        // a panic would fail the test.
        for columns in 1..=30u32 {
            for len in [400usize, 900, 1830, 2600] {
                let data = vec![b'a'; len];
                if let Ok(symbol) = pdf417_codec(2, Some(columns)).encode(&data) {
                    assert_eq!(
                        symbol.width(),
                        17 * columns as usize + 69,
                        "columns={columns} len={len}"
                    );
                }
            }
        }
    }

    #[test]
    fn pdf417_encodes_non_ascii_text_and_raw_bytes() {
        // Non-ASCII UTF-8 is declared with a UTF-8 ECI; non-UTF-8 bytes go through
        // the Latin-1 mapping. Both must encode without error.
        for data in [
            "你好，世界".as_bytes(),
            "参数 --file 文件".as_bytes(),
            &[0xff, 0x00, 0x80, 0x41, 0x42],
        ] {
            assert!(
                encode_multi(data, &pdf417(2), None, None).is_ok(),
                "failed for {data:?}"
            );
        }
    }

    #[test]
    fn pdf417_splits_an_oversized_payload() {
        let codes = encode_multi(&vec![b'a'; 4000], &pdf417(2), None, None).unwrap();
        assert!(codes.len() > 1, "expected more than one symbol");
    }

    #[test]
    fn pdf417_split_covers_the_payload() {
        let data: Vec<u8> = (0..4000u32).map(|i| b'a' + (i % 26) as u8).collect();
        let chunks = split_chunks_with(&pdf417_codec(2, None), &data, None, None);
        assert!(chunks.len() > 1, "expected a split");
        assert_eq!(chunks.concat(), data);
    }

    #[test]
    fn pdf417_reports_the_ceiling_for_an_oversized_input() {
        // High bytes force byte compaction, the least dense mode, so the ceiling
        // sits well below the numeric bound.
        let huge = vec![0xffu8; 4000];
        let Err(AppError::Input(message)) = pdf417_codec(2, None).encode(&huge) else {
            panic!("expected an input error");
        };
        assert!(message.contains("PDF417"), "{message}");
        assert!(message.contains("4000"), "{message}");
        assert!(message.contains("level 2"), "{message}");
    }

    #[test]
    fn pdf417_higher_levels_hold_less_data() {
        let data = vec![b'a'; 1500];
        assert!(pdf417_codec(0, None).fits(&data), "level 0");
        assert!(!pdf417_codec(8, None).fits(&data), "level 8");
    }

    #[test]
    fn multi_splits_an_oversized_payload() {
        let codes = encode_multi(&vec![b'x'; 4000], &qr(Ec::L), None, None).unwrap();
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
        let codes = encode_multi(&vec![b'a'; 250], &qr(Ec::L), Some(100), None).unwrap();
        assert_eq!(codes.len(), 3);
    }

    #[test]
    fn zero_max_bytes_is_floored_instead_of_panicking() {
        // Regression: a zero cap used to reach `div_ceil(0)` and panic. It is
        // floored to one byte, so the payload still splits (one byte, or one
        // whole character, per code) and no character is divided.
        let ascii = encode_multi(b"abc", &qr(Ec::L), Some(0), None).unwrap();
        assert_eq!(ascii.len(), 3, "one byte per code");

        let cjk = encode_multi("汉字".as_bytes(), &qr(Ec::L), Some(0), None).unwrap();
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
            let ours = build(sample.as_bytes(), Ec::M).unwrap();
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
        let ours = build(data, Ec::M).unwrap();
        let plain = QrCode::with_error_correction_level(data, ec_level(Ec::M)).unwrap();
        assert_eq!(ours.to_colors(), plain.to_colors());
    }

    #[test]
    fn non_ascii_encoding_is_declared_as_utf8() {
        // The UTF-8 ECI changes the bit stream, so the matrix differs from a
        // plain encoding of the same bytes.
        let data = "参数".as_bytes();
        let ours = build(data, Ec::M).unwrap();
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
        let ours = build(data, Ec::L).unwrap();
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
