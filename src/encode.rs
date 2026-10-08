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
            AppError::Input("input cannot be encoded: data too long".to_string())
        }
        _ => AppError::Input("input cannot be encoded as a QR code".to_string()),
    })
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
}
