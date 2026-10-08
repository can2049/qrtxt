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

/// Encode `data` into an ordinary QR code at the exact error level.
///
/// `qrcode` never boosts the error level, matching qrpipe's `boost_error=false`.
pub fn encode(data: &[u8], ec: Ec) -> Result<QrCode, AppError> {
    QrCode::with_error_correction_level(data, ec_level(ec)).map_err(|error| match error {
        QrError::DataTooLong => {
            let max = max_fitting_bytes(data, ec);
            AppError::Input(format!(
                "input too long: {} bytes (maximum {} bytes at error-correction level {ec:?})",
                data.len(),
                max
            ))
        }
        _ => AppError::Input("input cannot be encoded as a QR code".to_string()),
    })
}

/// The largest length any single QR symbol can hold (numeric mode, version 40-L).
const ABSOLUTE_MAX_BYTES: usize = 7089;

/// Largest prefix of `data`, in bytes, that still fits at `ec`.
///
/// Bounded by [`ABSOLUTE_MAX_BYTES`] so the search cost does not grow with an
/// arbitrarily large rejected input.
fn max_fitting_bytes(data: &[u8], ec: Ec) -> usize {
    let (mut lo, mut hi) = (0usize, data.len().min(ABSOLUTE_MAX_BYTES));
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if QrCode::with_error_correction_level(&data[..mid], ec_level(ec)).is_ok() {
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
}
