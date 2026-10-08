# qrtxt

Turn text into a QR code, right in your terminal.

**English** | [简体中文](README.zh-CN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`qrtxt` reads a string, a file, or piped input and prints an ordinary QR code
using Unicode block characters. No graphics environment is required — just a
UTF-8 terminal.

```console
$ qrtxt "hi"

     ▄▄▄▄▄ ██  ▄ █ ▄▄▄▄▄
     █   █ █  █▄ █ █   █
     █▄▄▄█ █ ▀█▄ █ █▄▄▄█
    ▄▄▄▄▄▄▄█ █ ▀ █▄▄▄▄▄▄▄
    ▄▀  ▄ ▄▀██▀█ █▄ ▄▄▄█▀
    ▀▀ ▄▀█▄▄█ █▄█▄█▀ ▄ ▄█
    ▄▄▄█▄█▄█▀██ ▀ ▀█▄▀▄█
     ▄▄▄▄▄ █▀▀ ▀ ▀ ▄█▀█▄▀
     █   █ █ ▀▀█ ██ ██ ██
     █▄▄▄█ █▄▀▀▄█▄█▀ ▄ ██
           ▀ ▀  ▀ ▀▀    ▀
```

(The quiet zone is rendered as blank space, so the code appears to float.)

## Features

- **Three input modes** — a literal argument, a file (`--file`), or standard input.
- **Exact error correction** — `L`, `M`, `Q`, or `H`, never silently boosted.
- **Three glyph sets** — half blocks (default), quadrants, braille — trading
  robustness for screen density.
- **ANSI rendering** — `--no-compact` for terminals without block glyphs.
- **Dark and light terminals** — `--invert` handles light backgrounds.
- **Safe by default** — no `unsafe` code, and errors never echo the payload.
- **A single static binary** — no runtime dependencies.

## Install

From a clone:

```console
git clone https://github.com/can2049/qrtxt
cd qrtxt
cargo build --release
# binary at target/release/qrtxt
```

Or install directly with Cargo:

```console
cargo install --path .
```

Requires Rust 1.85 or newer.

## Usage

```text
qrtxt [OPTIONS] [DATA]

Arguments:
  [DATA]                 Literal payload; when omitted, read from --file or stdin

Options:
  -f, --file <PATH>      Read the payload from a file
      --preserve-newline Do not strip one trailing newline from piped/file input
  -e, --error <LEVEL>    Exact error-correction level: L, M, Q, or H [default: M]
  -b, --border <N>       Quiet-zone width in modules [default: 4]
  -s, --scale <N>        Terminal module scale [default: 1]
      --invert           Invert the ink mapping, for light-background terminals
      --glyphs <SET>     Glyph set: half, quadrant, or braille [default: half]
      --no-compact       Use ANSI rendering instead of Unicode block characters
  -h, --help             Print help
  -V, --version          Print version
```

## Examples

```console
# literal string
qrtxt "https://example.com"

# pipe a secret to the screen without touching disk
cat token.txt | qrtxt

# read from a file
qrtxt --file payload.txt

# higher error correction
qrtxt --error H "important payload"

# a bigger symbol and a tighter quiet zone
qrtxt --scale 2 --border 1 "hello"

# denser packing for narrow terminals
qrtxt --glyphs braille "hello"

# light-background terminal
qrtxt --invert "hello"
```

## Glyph sets

| `--glyphs` | Modules per cell | Notes |
|---|---|---|
| `half`     | 1 × 2            | Default. Most robust across fonts. |
| `quadrant` | 2 × 2            | Twice the horizontal density. |
| `braille`  | 2 × 4            | Densest; depends on the font rendering dots tightly. |

`--scale` (physical size) and `--glyphs` (logical density) are independent knobs.

## Dark and light terminals

Block characters are drawn in the terminal's **foreground** color, so the default
output assumes a dark-background terminal. On a light-background terminal the
code would appear inverted, so pass `--invert` to restore a scannable
dark-on-light code. `qrtxt` never queries the terminal background itself.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `2` | Usage or input error (empty input, conflicting flags, data too long) |
| `1` | Runtime/IO error (unreadable file, write failure) |

A closed downstream pipe (for example `qrtxt ... | head`) is treated as success.

## How it works

```text
argv / stdin -> input::resolve -> encode::encode -> render::build_ink -> render::render -> stdout
```

The crate is layered so that dependencies point in one direction only:

- `cli` parses arguments and orchestrates the run (`run` / `run_with`).
- `input`, `encode`, and `render` are pure and do not depend on `clap`.
- `types` holds the shared value types; `error` maps failures to exit codes.

Rendering happens in two steps: `build_ink` turns the QR matrix into a boolean
"ink" grid (applying the quiet zone, scale, and inversion), and `render` packs
that grid into character cells per glyph set. The round-trip test suite renders
the output, reconstructs the module grid, and decodes it again with `rqrr` to
prove the printed code is still scannable.

## License

MIT. See [LICENSE](LICENSE).
