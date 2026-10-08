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
- **Long payloads** — input too long for one symbol is split automatically across
  balanced codes; `--max-size` caps the bytes per code, `--chunk` sets a floor on
  the number of codes.
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
  [DATA]                 Literal payload to encode

Options:
  -g, --glyphs <SET>              Glyph set used to pack modules into character cells: half (1x2 modules per cell; most robust), quadrant (2x2), or braille (2x4; densest) [default: half]
  -e, --error-correction <LEVEL>  Error-correction level: L (about 7% recoverable), M (15%), Q (25%), or H (30%) [default: L]
  -m, --max-size <BYTES>          Cap the payload of each QR code at BYTES bytes; implies splitting (default: the symbol's own limit)
  -c, --chunk <COUNT>             Split the payload across at least COUNT QR codes; advisory (default: no minimum)
  -a, --no-compact                Render with ANSI escape codes instead of Unicode block characters
  -f, --file <PATH>               Read the payload from a file
  -p, --preserve-newline          Keep one trailing newline from file or piped input
  -b, --border <BORDER>           Quiet-zone width in modules; 0 removes the margin [default: 4]
  -i, --invert                    Invert the ink, for light-background terminals
  -h, --help                      Print help (see more with '--help')
  -V, --version                   Print version
```

Run `qrtxt --help` for a full description of every option and the effect of its
arguments (`-h` prints the short summary).

## Examples

```console
# literal string
qrtxt "https://example.com"

# pipe a secret to the screen without touching disk
cat token.txt | qrtxt

# read from a file
qrtxt --file payload.txt

# higher error correction
qrtxt --error-correction H "important payload"

# a tighter quiet zone
qrtxt --border 1 "hello"

# denser packing for narrow terminals
qrtxt --glyphs braille "hello"

# light-background terminal
qrtxt --invert "hello"

# split automatically, capping each code at 500 bytes
qrtxt --max-size 500 --file big.txt

# split into at least 4 balanced codes
qrtxt --chunk 4 "a-short-but-verifiable-payload"
```

## Glyph sets

| `--glyphs` | Modules per cell | Notes |
|---|---|---|
| `half` (h)     | 1 × 2        | Default. Most robust across fonts. |
| `quadrant` (q) | 2 × 2        | Twice the horizontal density. |
| `braille` (b)  | 2 × 4        | Densest; depends on the font rendering dots tightly. |

Each glyph set also accepts its first letter (`-g h`, `-g q`, `-g b`).

## Dark and light terminals

Block characters are drawn in the terminal's **foreground** color, so the default
output assumes a dark-background terminal. On a light-background terminal the
code would appear inverted, so pass `--invert` to restore a scannable
dark-on-light code. `qrtxt` never queries the terminal background itself.

## Long payloads

One QR symbol holds a bounded amount of data (about 2953 bytes at level `L`, less
at higher levels). When the payload does not fit, `qrtxt` splits it across several
codes automatically — no flag needed — printing them in order under a `QR i/N`
caption. The chunks are balanced, so every code carries a comparable amount of
data, and cuts fall on character boundaries, so multi-byte characters are never
broken between codes. A payload that fits one symbol is printed as a single,
uncaptioned code.

Pass `--max-size BYTES` to cap the payload in each code (for smaller, easier-to-scan
symbols). It implies splitting: even a payload that would fit one symbol is split
so every code stays within the cap. A cap larger than a symbol can hold is clamped
down to the symbol's own limit.

Pass `--chunk COUNT` to spread the payload across **at least** COUNT codes (for
example, to keep every code small or to lay them out on a page). It is advisory:
the payload is split into `COUNT` balanced codes when the content allows, else into
as many as possible (one code per character is the ceiling). `--chunk` and
`--max-size` can be combined; whichever forces more codes wins.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `2` | Usage or input error (empty input, conflicting flags) |
| `1` | Runtime/IO error (unreadable file, write failure) |

A closed downstream pipe (for example `qrtxt ... | head`) is treated as success.

## How it works

```text
argv / stdin -> input::resolve -> encode::encode_multi -> render::build_ink -> render::render -> stdout
```

The crate is layered so that dependencies point in one direction only:

- `cli` parses arguments and orchestrates the run (`run` / `run_with`).
- `input`, `encode`, and `render` are pure and do not depend on `clap`.
- `types` holds the shared value types; `error` maps failures to exit codes.

Rendering happens in two steps: `build_ink` turns the QR matrix into a boolean
"ink" grid (applying the quiet zone and inversion), and `render` packs that grid
into character cells per glyph set. The round-trip test suite renders the output,
reconstructs the module grid, and decodes it again with `rqrr` to prove the printed
code is still scannable.

## License

MIT. See [LICENSE](LICENSE).
