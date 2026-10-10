# qrtxt

Turn text into a QR code, right in your terminal.

**English** | [简体中文](README.zh-CN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/qrtxt.svg)](https://crates.io/crates/qrtxt)

`qrtxt` reads a string, a file, or piped input and prints an ordinary QR code.
By default it picks the best rendering your terminal offers — a crisp bitmap
over the Kitty or Sixel graphics protocol where supported, otherwise Unicode
block characters, so a plain UTF-8 terminal always works.

```console
$ qrtxt "hi"

     ▄▄▄▄▄ █▀▄█▀▄█ ▄▄▄▄▄
     █   █ █▀▄█▀██ █   █
     █▄▄▄█ █▀▀ ▄ █ █▄▄▄█
    ▄▄▄▄▄▄▄█▄█ ▀ █▄▄▄▄▄▄▄
     ▄ ▄▄▀▄▄▄▄▀█ ▄█ █▄█▄▀
    ▀ ▀▀█ ▄ ██▄▄█▄█▀ ▄ ▄█
    ▄█▄█▄▄▄█▀▄▀ ▀ ▀█▄▀▄█
     ▄▄▄▄▄ █▄▀▀▀ ▀ ▄█▀█▄▀
     █   █ █ ▀▄█ ██ ██ ██
     █▄▄▄█ █ ▀█▄█▄█▀ ▄ ██
           ▀    ▀ ▀▀    ▀
```

(The quiet zone is rendered as blank space, so the code appears to float.)

## Features

- **Three input modes** — a literal argument, a file (`--file`), or standard input.
- **Exact error correction** — `L`, `M`, `Q`, or `H`, never silently boosted.
- **Five render modes** — `--mode auto` (default) picks the best rendering the
  terminal offers: a crisp bitmap over the Kitty or Sixel graphics protocol where
  supported, otherwise Unicode block glyphs. Force a specific one with `text`,
  `ansi`, `kitty`, or `sixel`. A bitmap is smaller and crisper than block glyphs;
  `ansi` covers terminals without block glyphs; an unsupported bitmap mode fails
  fast instead of printing garbage.
- **Three glyph sets** — half blocks (default), quadrants, braille — trading
  robustness for screen density (used by `--mode text`).
- **Dark and light terminals** — `--invert` handles light backgrounds.
- **Long payloads** — input too long for one symbol is split automatically across
  balanced codes; `--max-bytes` caps the bytes per code, `--chunk` sets a floor on
  the number of codes.
- **Safe by default** — no `unsafe` code, and errors never echo the payload.
- **A single static binary** — no runtime dependencies.

## Install

`qrtxt` is published on [crates.io](https://crates.io/crates/qrtxt). Install it
straight from there:

```console
cargo install qrtxt
```

From a clone:

```console
git clone https://github.com/can2049/qrtxt
cd qrtxt
cargo build --release
# binary at target/release/qrtxt
```

Or install a local checkout with Cargo:

```console
cargo install --path .
```

Requires Rust 1.85 or newer.

## Usage

```text
qrtxt [OPTIONS] [DATA]

Options:
  -h, --help     Print help (see more with '--help')
  -V, --version  Print version

Payload:
  -f, --file <PATH>       Read the payload from a file
  -p, --preserve-newline  Keep one trailing newline from file or piped input
  [DATA]                  Literal payload to encode

Encoding:
  -e, --error-correction <LEVEL>  Error-correction level: L (about 7% recoverable), M (15%), Q (25%), or H (30%) [default: L]
  -m, --max-bytes <BYTES>         Cap the payload of each QR code at BYTES bytes; implies splitting (default: the symbol's own limit)
  -c, --chunk <COUNT>             Split the payload across at least COUNT QR codes; advisory (default: no minimum)

Rendering:
  -M, --mode <MODE>      How to draw the code: auto, text, ansi, kitty, or sixel [default: auto]
  -b, --border <BORDER>  Quiet-zone width in modules; 0 removes the margin [default: 4]
  -i, --invert           Invert the ink, for light-background terminals

Text rendering:
  -g, --glyphs <SET>  Glyph set used to pack modules into character cells: half (h; 1x2 modules per cell, most robust), quadrant (q; 2x2), or braille (b; 2x4, densest) [default: half]

Source: https://github.com/can2049/qrtxt
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

# a crisp black-and-white bitmap (needs a Kitty-protocol terminal)
qrtxt --mode kitty "https://example.com"

# a crisp black-and-white bitmap (needs a Sixel-protocol terminal)
qrtxt --mode sixel "https://example.com"

# force Unicode block glyphs (for a pipe, or to compare against a bitmap)
qrtxt --mode text "hello"

# ANSI escape codes for terminals without block glyphs
qrtxt --mode ansi "hello"

# higher error correction
qrtxt --error-correction H "important payload"

# a tighter quiet zone
qrtxt --border 1 "hello"

# denser packing for narrow terminals
qrtxt --mode text --glyphs braille "hello"

# light-background terminal
qrtxt --invert "hello"

# split automatically, capping each code at 500 bytes
qrtxt --max-bytes 500 --file big.txt

# split into at least 4 balanced codes
qrtxt --chunk 4 "a-short-but-verifiable-payload"
```

## Rendering modes

`--mode` chooses how the code is drawn:

| `--mode` | Family | What it draws |
|---|---|---|
| `auto` (default) | — | A bitmap when the terminal supports one, otherwise Unicode block glyphs. |
| `text` | characters | Unicode block glyphs (the glyph set applies). |
| `ansi` | characters | Reverse-video escape codes; wider, for terminals without block glyphs. |
| `kitty` | bitmap | A black-and-white bitmap via the Kitty graphics protocol. |
| `sixel` | bitmap | The same bitmap via the Sixel graphics protocol. |

A bitmap is smaller and crisper than block glyphs, so `auto` prefers it — but only
where it makes sense. `auto` **never** emits a bitmap when standard output is not
a terminal, so `qrtxt ... > code.txt` or `qrtxt ... | head` always produces text.
`--mode kitty` and `--mode sixel` force a protocol and fail with a usage error
(exit code 2) when the terminal cannot draw it.

`--glyphs` applies to `text` only; `--invert` applies to every mode (in a bitmap
it swaps the two colours). `--help` probes the current terminal once and appends
to the `--mode` entry what `auto` will pick there.

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

## Kitty graphics protocol

`--mode kitty` draws the code as a real bitmap using the
[Kitty graphics protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
instead of Unicode glyphs, so it is not limited by how the font renders block
characters and can be much smaller. It needs a terminal that implements the
protocol — kitty, Ghostty,
and WezTerm, for example. `qrtxt` probes the terminal with an `a=q` handshake
first; if the protocol is unsupported (or standard output is not a terminal) it
stops with a usage error rather than emit escape bytes the terminal cannot
render. The output is pure black and white and carries its own white quiet zone,
so `--invert` swaps the two colours while the code stays self-contained; the
glyph set is ignored.

`auto` uses the same probe to prefer Kitty over Sixel, and the `--help` probe
appends to the `--mode` entry what `auto` will pick for the current terminal.

## Sixel graphics protocol

`--mode sixel` draws the same black-and-white bitmap as `--mode kitty`, but through
the older [Sixel graphics format](https://en.wikipedia.org/wiki/Sixel). The two
cover different terminals: Sixel reaches xterm, Konsole, foot, mlterm, iTerm2, and
Windows Terminal (1.22+), which do not implement the Kitty protocol, while kitty
and Ghostty implement Kitty but not Sixel. Because a QR code has only two colours,
the Sixel output uses two palette registers and run-length encoding, so it is
compact.

Support detection is weaker than Kitty's: Sixel has no handshake, so `qrtxt` sends
a Primary Device Attributes query and only accepts terminals that advertise Sixel
(device attribute `4`). A terminal that supports Sixel without advertising it is
treated as unsupported, so the tool never prints escape bytes a terminal cannot
draw. As with `--mode kitty`, the output is pure black and white with its own white
quiet zone, `--invert` swaps the two colours, and the glyph set is ignored. The
`--help` probe reports it on the `--mode` entry the same way.

The image declares square pixels, so the modules stay square on terminals that
honour the raster attributes — but Sixel rendering has not been visually verified
on every terminal, so check that a printed code still scans.

## Long payloads

One QR symbol holds a bounded amount of data (about 2953 bytes at level `L`, less
at higher levels). When the payload does not fit, `qrtxt` splits it across several
codes automatically — no flag needed — printing them in order under a `QR i/N`
caption. The chunks are balanced, so every code carries a comparable amount of
data, and cuts fall on character boundaries, so multi-byte characters are never
broken between codes. A payload that fits one symbol is printed as a single,
uncaptioned code.

Pass `--max-bytes BYTES` to cap the payload in each code (for smaller, easier-to-scan
symbols). It implies splitting: even a payload that would fit one symbol is split
so every code stays within the cap. A cap larger than a symbol can hold is clamped
down to the symbol's own limit.

Pass `--chunk COUNT` to spread the payload across **at least** COUNT codes (for
example, to keep every code small or to lay them out on a page). It is advisory:
the payload is split into `COUNT` balanced codes when the content allows, else into
as many as possible (one code per character is the ceiling). `--chunk` and
`--max-bytes` can be combined; whichever forces more codes wins.

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
- `kitty` and `sixel` hold the low-level protocol framing, shared by `cli` (the
  terminal probes) and `render` (bitmap transmission).
- `types` holds the shared value types; `error` maps failures to exit codes.

Rendering happens in two steps: `build_ink` turns the QR matrix into a boolean
"ink" grid (applying the quiet zone and inversion), and `render` packs that grid
into character cells per glyph set. `render_kitty` and `render_sixel` bypass the
grid and emit a black-and-white bitmap over the Kitty or Sixel protocol. The
round-trip test suite renders the output, reconstructs the module grid, and
decodes it again with `rqrr` to prove the printed code is still scannable.

## License

MIT. See [LICENSE](LICENSE).
