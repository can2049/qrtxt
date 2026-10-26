# qrtxt

Turn text into a QR code, right in your terminal.

**English** | [简体中文](README.zh-CN.md)

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/qrtxt.svg)](https://crates.io/crates/qrtxt)

`qrtxt` reads a string, a file, or piped input and prints an ordinary QR code.
The `datamatrix` subcommand prints a Data Matrix (ECC 200) symbol, `pdf417` a
stacked PDF417 symbol, and `aztec` an Aztec Code symbol instead. By
default it picks the best rendering your terminal offers — a crisp bitmap over
the Kitty or Sixel graphics protocol where supported, otherwise Unicode block
characters, so a plain UTF-8 terminal always works.

```console
$ qrtxt "hi"

    █▀▀▀▀▀█ ▄▀ ▄▀ █▀▀▀▀▀█
    █ ███ █ ▄▀ ▄  █ ███ █
    █ ▀▀▀ █ ▄▄█▀█ █ ▀▀▀ █
    ▀▀▀▀▀▀▀ ▀ █▄█ ▀▀▀▀▀▀▀
    █▀█▀▀▄▀▀▀▀▄ █▀ █ ▀ ▀▄
    ▄█▄▄ █▀█  ▀▀ ▀ ▄█▀█▀
    ▀ ▀ ▀▀▀ ▄▀▄█▄█▄ ▀▄▀ █
    █▀▀▀▀▀█ ▀▄▄▄█▄█▀ ▄ ▀▄
    █ ███ █ █▄▀ █  █  █
    █ ▀▀▀ █ █▄ ▀ ▀ ▄█▀█
    ▀▀▀▀▀▀▀ ▀▀▀▀ ▀  ▀▀▀▀
```

(The character renderers set explicit colors, so dark modules print black on a
white field with a white quiet zone, whatever the terminal's own background is. The
quiet zone shows as blank space in this color-free transcript.)

## Features

- **Three input modes** — a literal argument, a file (`--file`), or standard input.
- **Four barcode formats** — one subcommand each: `qr` (an ordinary QR code),
  `datamatrix` (a Data Matrix (ECC 200) symbol), `pdf417` (a stacked PDF417
  symbol), and `aztec` (an Aztec Code symbol).
- **Per-format error correction** — the `qr` subcommand takes `L`, `M`, `Q`, or
  `H` (never silently boosted), `pdf417` its numeric levels `0`–`8`, and `aztec`
  a percentage `0`–`100`; a Data Matrix's correction is fixed by its symbol size,
  so `datamatrix` has no such option. `aztec` also takes `--layers` to pin the
  symbol shape.
- **Six render modes** — `--mode auto` (default) picks the best rendering the
  terminal offers: a crisp bitmap over the Kitty or Sixel graphics protocol where
  supported, otherwise Unicode block glyphs. Force one with `text`, `ansi`,
  `bitmap`, `kitty`, or `sixel`; `bitmap` forces an image and picks Kitty or Sixel
  by what the terminal supports. A bitmap is smaller and crisper than block glyphs;
  `ansi` covers terminals without block glyphs; an unsupported bitmap mode fails
  fast instead of printing garbage.
- **Three glyph sets** — half blocks (default), quadrants, braille — trading
  robustness for screen density (used by `--mode text`).
- **Readable on any terminal** — the character renderers set explicit colors and
  print dark modules black on a white field (with a white quiet zone), so the code
  scans on a dark- or light-background terminal; `--invert` swaps to light-on-dark.
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
$ qrtxt -h
Turn text into a terminal QR code, Data Matrix, PDF417, or Aztec Code

Usage: qrtxt [COMMAND] [OPTIONS] [DATA]

Commands:
  qr          Encode a QR code
  datamatrix  Encode a Data Matrix (ECC 200) symbol [alias: dm]
  pdf417      Encode a PDF417 symbol [alias: pdf]
  aztec       Encode an Aztec Code symbol [alias: az]
  help        Print this message or the help of the given subcommand(s)

Options:
  -h, --help     Print help (see more with '--help')
  -V, --version  Print version

Payload:
  -f, --file <PATH>       Read the payload from a file
  -p, --preserve-newline  Keep one trailing newline from file or piped input
  [DATA]                  Literal payload to encode

Encoding:
  -m, --max-bytes <BYTES>  Cap the payload of each symbol at BYTES bytes; implies splitting (default: the symbol's own limit)
  -c, --chunk <COUNT>      Split the payload across at least COUNT symbols; advisory (default: no minimum)

Rendering:
  -M, --mode <MODE>      How to draw the code [default: auto] [possible values: auto, text, ansi, bitmap, kitty, sixel]
  -b, --border <BORDER>  Quiet-zone width in modules; 0 removes the margin [default: 4]
  -i, --invert           Invert the colors: draw a light-on-dark code instead of dark-on-light

Text rendering:
  -g, --glyphs <SET>  Glyph set used to pack modules into character cells [default: half] [possible values: half, quadrant, braille]

With no COMMAND a QR code is produced, so `qrtxt DATA` and `qrtxt qr DATA` do the same. Options are written after the command.

Examples:
  qrtxt "https://example.com"   A QR code (the default)
  cat token.txt | qrtxt         Encode piped input
  qrtxt datamatrix "hi"         A Data Matrix symbol
  qrtxt pdf417 -C 1 "hi"        A narrow, tall PDF417
  qrtxt aztec --layers -3 "hi"  A compact Aztec Code
  qrtxt --mode text "hi"        Force Unicode block glyphs
  qrtxt --mode bitmap "hi"      Force a bitmap (Kitty or Sixel)

Source: https://github.com/can2049/qrtxt
```

A bare invocation with no subcommand is treated as `qr`, so `qrtxt "hi"` and
`cat token.txt | qrtxt` keep working. The shared options are listed under both
`qrtxt -h` and every subcommand; `-e/--error-correction` is per-format (`qr` takes
`L`/`M`/`Q`/`H`, `pdf417` takes `0`–`8`, `aztec` a percentage `0`–`100`), so run
`qrtxt qr --help`, `qrtxt datamatrix --help`, `qrtxt pdf417 --help`, or
`qrtxt aztec --help` for the options of one symbology, and `qrtxt qr -h` for the
short summary.

## Examples

```console
# literal string (a bare invocation defaults to qr)
qrtxt "https://example.com"

# the same, spelled explicitly
qrtxt qr "https://example.com"

# pipe a secret to the screen without touching disk
cat token.txt | qrtxt

# read from a file
qrtxt qr --file payload.txt

# a Data Matrix (ECC 200) symbol instead of a QR code
qrtxt datamatrix "https://example.com"

# a stacked PDF417 symbol
qrtxt pdf417 "https://example.com"

# a higher PDF417 error-correction level (0-8)
qrtxt pdf417 --error-correction 6 "important payload"

# a narrower, taller PDF417 (fits a narrow terminal)
qrtxt pdf417 -C 1 "https://example.com"

# the same, packed into half the cells
qrtxt pdf417 --cl 1 --mode text --glyphs braille "https://example.com"

# an Aztec Code symbol
qrtxt aztec "https://example.com"

# an Aztec Code with more error correction and a fixed layer count
qrtxt aztec --error-correction 60 --layers 5 "important payload"

# a compact Aztec Code (negative layer count)
qrtxt aztec --layers -3 "https://example.com"

# a crisp black-and-white bitmap (needs a Kitty-protocol terminal)
qrtxt qr --mode kitty "https://example.com"

# a crisp black-and-white bitmap (needs a Sixel-protocol terminal)
qrtxt qr --mode sixel "https://example.com"

# a bitmap, whichever of Kitty or Sixel the terminal supports
qrtxt qr --mode bitmap "https://example.com"

# force Unicode block glyphs (for a pipe, or to compare against a bitmap)
qrtxt qr --mode text "hello"

# ANSI escape codes for terminals without block glyphs
qrtxt qr --mode ansi "hello"

# higher error correction (qr only)
qrtxt qr --error-correction H "important payload"

# a tighter quiet zone
qrtxt qr --border 1 "hello"

# denser packing for narrow terminals
qrtxt qr --mode text --glyphs braille "hello"

# light-on-dark instead of the default dark-on-light
qrtxt qr --invert "hello"

# split automatically, capping each code at 500 bytes
qrtxt qr --max-bytes 500 --file big.txt

# split into at least 4 balanced codes
qrtxt qr --chunk 4 "a-short-but-verifiable-payload"
```

## Barcode formats

The subcommand chooses which symbol is produced; `--mode` (below) chooses how it
is drawn. Every format renders through every mode.

| Subcommand | Symbol |
|---|---|
| `qr` (default) | An ordinary QR code. `--error-correction` takes `L`/`M`/`Q`/`H`. |
| `datamatrix` (`dm`) | A Data Matrix (ECC 200) symbol, always square. |
| `pdf417` (`pdf`) | A stacked PDF417 symbol. `--error-correction` takes `0`–`8`. |
| `aztec` (`az`) | An Aztec Code symbol. `--error-correction` takes `0`–`100`; `--layers` pins the shape. |

A bare invocation with no subcommand is treated as `qr`, so `qrtxt "hi"` and
`cat token.txt | qrtxt` keep working. A Data Matrix's error correction is fixed
by the symbol size, so `datamatrix` exposes no options and rejects
`--error-correction`.

A Data Matrix holds far less data per symbol than a QR code (about 1555 bytes
versus 2953 at the lowest error level), so the same payload splits into more
symbols. PDF417 holds less still — around 1068 bytes at its default level, or
about 1075 at level 0 — and its error-correction levels work the opposite way to
QR's: each step from `0` to `8` roughly doubles the correction codewords, so a
higher level is more robust but holds less. Non-ASCII text is declared with a
UTF-8 ECI, exactly as for QR; many scanners handle that inconsistently, so prefer
printable ASCII where you can. Any payload a symbol cannot represent as UTF-8
text — for instance raw binary — is encoded through a Latin-1 mapping that
round-trips the original bytes.

A PDF417 symbol is a stack of rows, so its shape is a width/height trade-off:
by default the encoder picks a balanced one, which tends to be wide and short.
Pass `--columns N` (1–30) to force the data-column count — the symbol is
`17 × N + 69` modules wide and grows taller as `N` falls. `--columns 1` gives the
narrowest, tallest code; larger values give wider, shorter ones. `-C` and `--cl`
are accepted as short forms. For terminal
width, `--mode text --glyphs quadrant` (or `braille`) also packs two modules per
cell and roughly halves the printed width.

## Rendering modes

`--mode` chooses how the code is drawn:

| `--mode` | Family | What it draws |
|---|---|---|
| `auto` (default) | — | A bitmap when the terminal supports one, otherwise Unicode block glyphs. |
| `text` | characters | Unicode block glyphs (the glyph set applies). |
| `ansi` | characters | Black-on-white escape codes; wider, for terminals without block glyphs. |
| `bitmap` | bitmap | A bitmap via whichever of Kitty or Sixel the terminal supports. |
| `kitty` | bitmap | A black-and-white bitmap via the Kitty graphics protocol. |
| `sixel` | bitmap | The same bitmap via the Sixel graphics protocol. |

A bitmap is smaller and crisper than block glyphs, so `auto` prefers it — but only
where it makes sense. `auto` **never** emits a bitmap when standard output is not
a terminal, so `qrtxt ... > code.txt` or `qrtxt ... | head` always produces text.
`--mode bitmap` forces a bitmap and picks Kitty or Sixel by terminal support;
`--mode kitty` and `--mode sixel` force one protocol. All three fail with a usage
error (exit code 2) when the terminal cannot draw it.

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

The character renderers (`text` and `ansi`) set explicit colors: the field — the
light modules and the quiet zone — is white and the dark modules are black. The
code is therefore a self-contained dark-on-white image that scans whatever the
terminal's own background and colors are, including a **black-background**
terminal, where a renderer that borrowed the terminal colors would leave the quiet
zone dark. Pass `--invert` to swap the two colors and get a light-on-dark code
instead. The bitmap modes carry their own white quiet zone the same way. `qrtxt`
never queries the terminal background itself. These color codes are part of the text
output, so a piped or redirected `text`/`ansi` run carries them too — strip them if
you need plain glyphs.

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

`auto` and `bitmap` use the same probe to prefer Kitty over Sixel, and the
`--help` probe appends to the `--mode` entry what `auto` will pick for the current
terminal.

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
draw. The probe runs on Windows too — the replies are read from the console input
queue — so `auto` picks Sixel in Windows Terminal (verified with 1.24). As with
`--mode kitty`, the output is pure black and white with its own white quiet zone,
`--invert` swaps the two colours, and the glyph set is ignored. The `--help` probe
reports it on the `--mode` entry the same way.

The image declares square pixels, so the modules stay square on terminals that
honour the raster attributes — but Sixel rendering has not been visually verified
on every terminal, so check that a printed code still scans.

## Long payloads

One symbol holds a bounded amount of data (a QR code about 2953 bytes at level
`L`, less at higher levels). When the payload does not fit, `qrtxt` splits it across several
codes automatically — no flag needed — printing them in order under a `QR Code i/N`
(or `Data Matrix i/N`, `PDF417 i/N`) caption. The chunks are balanced, so every
code carries a comparable amount of data, and cuts fall on character boundaries,
so multi-byte characters are never broken between codes. A payload that fits one
symbol is printed as a single, uncaptioned code.

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
argv / stdin -> input::resolve -> encode::encode_multi -> render::build_ink -> render::render_on_field -> stdout
```

The crate is layered so that dependencies point in one direction only:

- `cli` parses arguments (`parse`) and orchestrates the run (`run` / `run_with`).
- `input`, `encode`, and `render` are pure and do not depend on `clap`.
- `encode` wraps one barcode crate per symbology (`qrcode` for QR, `datamatrix`
  for Data Matrix, `rxing` for PDF417) and emits a symbology-agnostic `Symbol` —
  a dark/light module grid — so only `encode` depends on a barcode library.
- `kitty` and `sixel` hold the low-level protocol framing, shared by `cli` (the
  terminal probes) and `render` (bitmap transmission).
- `types` holds the shared value types (including `Symbol`); `error` maps failures
  to exit codes.

Rendering happens in two steps: `build_ink` turns a `Symbol` into a boolean "ink"
grid (applying the quiet zone and inversion), and `render` packs that grid into
character cells per glyph set. `render_on_field` wraps those glyphs in explicit
colors so dark modules print black on a white field, and `render_ansi` paints the
same grid with escape codes. `render_kitty` and `render_sixel` bypass the grid and
emit a black-and-white bitmap over the Kitty or Sixel protocol. Each strategy lives in
its own `render` submodule (`frame`, `text`, `ansi`, `bitmap`) behind the `render_all`
dispatch. The round-trip test
suite renders the output, strips the colour codes, reconstructs the module grid, and
decodes it again — with `rqrr` for QR, the `datamatrix` crate for Data Matrix, and
`rxing` for PDF417 — to prove the printed code is still scannable.

## License

MIT. See [LICENSE](LICENSE).
