//! Kitty graphics protocol framing (FR-3.12).
//!
//! Pure byte-level helpers for the [Kitty graphics protocol]: they only ever
//! touch a caller-provided [`Write`], so the terminal probe and the image
//! placement live in [`crate::cli`] and [`crate::render`] respectively. Nothing
//! here depends on `qrcode`.
//!
//! [Kitty graphics protocol]: https://sw.kovidgoyal.net/kitty/graphics-protocol/

use std::io::{self, Write};

/// APC (application program command) introducer that starts every sequence: `ESC _ G`.
const APC_START: &[u8] = b"\x1b_G";
/// String terminator that ends every sequence: `ESC \`.
const APC_END: &[u8] = b"\x1b\\";

/// Maximum raw (pre-base64) bytes per chunk.
///
/// The protocol caps a chunk at 4096 base64 characters, which is 3072 raw bytes.
const CHUNK_RAW: usize = 3072;

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Encode `data` as standard base64 (RFC 4648, with `=` padding).
#[must_use]
pub fn encode_base64(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let first = u32::from(chunk[0]);
        let second = u32::from(chunk.get(1).copied().unwrap_or(0));
        let third = u32::from(chunk.get(2).copied().unwrap_or(0));
        let triple = (first << 16) | (second << 8) | third;
        out.push(BASE64[((triple >> 18) & 0x3f) as usize] as char);
        out.push(BASE64[((triple >> 12) & 0x3f) as usize] as char);
        out.push(if chunk.len() > 1 {
            BASE64[((triple >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            BASE64[(triple & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    out
}

/// Write `rgb` (`width` x `height`, three bytes per pixel) and display it at the
/// cursor: transmit-and-display (`a=T`), raw RGB (`f=24`), chunked as the protocol
/// requires.
pub fn transmit_rgb(
    rgb: &[u8],
    width: usize,
    height: usize,
    id: u32,
    out: &mut dyn Write,
) -> io::Result<()> {
    debug_assert_eq!(
        rgb.len(),
        width * height * 3,
        "RGB buffer must match the given dimensions"
    );

    let mut offset = 0;
    let mut first = true;
    loop {
        let end = (offset + CHUNK_RAW).min(rgb.len());
        let more = u8::from(end < rgb.len());
        let payload = encode_base64(&rgb[offset..end]);

        out.write_all(APC_START)?;
        if first {
            write!(out, "a=T,f=24,s={width},v={height},i={id},m={more};")?;
            first = false;
        } else {
            write!(out, "m={more};")?;
        }
        out.write_all(payload.as_bytes())?;
        out.write_all(APC_END)?;

        offset = end;
        if more == 0 {
            break;
        }
    }
    Ok(())
}

/// The support-probe sequence: a `a=q` query carrying a single black pixel.
#[must_use]
pub fn query(id: u32) -> Vec<u8> {
    // `s=v=1`, `f=24` => one pixel (three zero bytes) whose base64 is "AAAA".
    format!("\x1b_Gi={id},a=q,t=d,f=24,s=1,v=1;AAAA\x1b\\").into_bytes()
}

/// Whether a probe response reports success (the terminal replied `;OK`).
#[must_use]
pub fn response_ok(response: &[u8]) -> bool {
    response.windows(3).any(|window| window == b";OK")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let cases: &[(&[u8], &str)] = &[
            (b"", ""),
            (b"f", "Zg=="),
            (b"fo", "Zm8="),
            (b"foo", "Zm9v"),
            (b"foob", "Zm9vYg=="),
            (b"fooba", "Zm9vYmE="),
            (b"foobar", "Zm9vYmFy"),
        ];
        for (input, expected) in cases {
            assert_eq!(encode_base64(input), *expected, "input={input:?}");
        }
    }

    #[test]
    fn transmit_single_chunk_has_all_control_keys() {
        let rgb = vec![0u8; 2 * 2 * 3];
        let mut out = Vec::new();
        transmit_rgb(&rgb, 2, 2, 7, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.starts_with("\x1b_G"), "{text:?}");
        assert!(text.ends_with("\x1b\\"), "{text:?}");
        assert!(text.contains("a=T,f=24,s=2,v=2,i=7,m=0;"), "{text:?}");
        assert_eq!(text.matches("\x1b_G").count(), 1, "expected one chunk");
    }

    #[test]
    fn transmit_chunks_and_flags_more() {
        // 65 * 16 * 3 = 3120 raw bytes => two chunks (3072 + 48).
        let rgb = vec![0u8; 65 * 16 * 3];
        let mut out = Vec::new();
        transmit_rgb(&rgb, 65, 16, 1, &mut out).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(text.matches("\x1b_G").count(), 2, "expected two chunks");
        let chunks: Vec<&str> = text.split("\x1b_G").skip(1).collect();
        assert!(
            chunks[0].contains("a=T,f=24,s=65,v=16,i=1,m=1;"),
            "{text:?}"
        );
        assert!(chunks[1].starts_with("m=0;"), "{text:?}");
    }

    #[test]
    fn query_is_the_documented_probe() {
        assert_eq!(
            query(1),
            b"\x1b_Gi=1,a=q,t=d,f=24,s=1,v=1;AAAA\x1b\\".to_vec()
        );
    }

    #[test]
    fn response_ok_detects_success_only() {
        assert!(response_ok(b"\x1b_Gi=1;OK\x1b\\"));
        assert!(!response_ok(b"\x1b_Gi=1;ENOTSUP\x1b\\"));
        assert!(!response_ok(b""));
    }
}
