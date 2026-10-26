# qrtxt-winprobe

Safe Rust API for querying the Windows console: queries are written to standard
output and the terminal's replies are collected from `CONIN$`.

This is the Windows transport used by [qrtxt](https://github.com/can2049/qrtxt).
The Win32 calls are confined to one small internal module where each function
opts into `unsafe`; the polling loop, UTF-16 decoding, and reply handling are
safe code and unit-tested on every platform.

```rust
use std::time::Duration;

let reply = qrtxt_winprobe::probe(
    &[b"\x1b[c"],
    Duration::from_millis(300),
    &|bytes| bytes.ends_with(b"c"),
)?;
# Ok::<(), std::io::Error>(())
```

The caller must ensure standard output is a terminal before probing, otherwise
the escape sequences go into whatever stream stdout is connected to.

License: MIT.
