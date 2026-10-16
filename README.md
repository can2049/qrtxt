# qrtxt

Turn text into a QR code, right in your terminal.

在终端里把文字变成二维码。

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/qrtxt.svg)](https://crates.io/crates/qrtxt)

`qrtxt` reads a string, a file, or piped input and prints an ordinary QR code.
The `datamatrix` subcommand prints a Data Matrix (ECC 200) symbol, `pdf417` a
stacked PDF417 symbol, and `aztec` an Aztec Code symbol instead. By
default it picks the best rendering your terminal offers — a crisp bitmap over
the Kitty or Sixel graphics protocol where supported, otherwise Unicode block
characters, so a plain UTF-8 terminal always works.

`qrtxt` 读取字符串、文件或管道输入,在标准输出上打印一个普通二维码;用
`datamatrix` 子命令则打印 Data Matrix(ECC 200)符号,用 `pdf417` 子命令则打印堆叠式
PDF417 符号,用 `aztec` 子命令则打印 Aztec Code 符号。默认它会挑选终端所能提供
的最佳渲染方式——终端支持时用 Kitty/Sixel 图形协议绘制清晰的位图,否则回退到
Unicode 块字符,因此普通 UTF-8 终端始终可用。

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

(字符渲染器会显式设置颜色:深色模块在黑字白底上打印、静默区保持白色,因此不受终端
自身背景色影响;上面这段无色转写里静默区显示为空白。)

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

## 特性

- **三种输入方式** —— 位置参数字面量、文件(`--file`)、标准输入。
- **四种条码格式** —— 各对应一个子命令:`qr`(普通二维码)、`datamatrix`(Data Matrix(ECC 200)符号)、`pdf417`(堆叠式 PDF417 符号)与 `aztec`(Aztec Code 符号)。
- **按格式区分的纠错** —— `qr` 取 `L`、`M`、`Q`、`H`(不做自动升档),`pdf417` 取其数字等级 `0`–`8`,`aztec` 取百分比 `0`–`100`;Data Matrix 的纠错由符号尺寸固定,因此 `datamatrix` 没有该选项。`aztec` 还可用 `--layers` 固定符号形状。
- **六种渲染模式** —— `--mode auto`(默认)挑选终端的最佳渲染方式:支持时用
  Kitty/Sixel 图形协议绘制清晰位图,否则回退到 Unicode 块字符;也可用 `text`、
  `ansi`、`bitmap`、`kitty`、`sixel` 强制指定。`bitmap` 强制位图并依据终端支持
  在 Kitty/Sixel 中择优。位图比块字符更小更清晰,`ansi` 覆盖不支持块字符的终端,
  不受支持的位图模式会快速报错而不是打印乱码。
- **三种字形集** —— 半块(默认)、象限块、盲文点阵,在可扫性与屏幕密度之间取舍
  (仅用于 `--mode text`)。
- **任意终端都可读** —— 字符渲染器显式设置颜色,把深色模块打印成白底黑字(静默区为白色),
  因此在深色或浅色背景终端上都能扫;`--invert` 可换成浅底深字(反相)。
- **超长内容** —— 放不进单个二维码的内容会自动均分成多个二维码;`--max-bytes` 可限制每个二维码的字节数,`--chunk` 可指定二维码数量的下限。
- **默认安全** —— 无 `unsafe` 代码,错误信息绝不回显 payload。
- **单一静态二进制** —— 无运行时依赖。

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

## 安装

`qrtxt` 已发布到 [crates.io](https://crates.io/crates/qrtxt),可直接安装:

```console
cargo install qrtxt
```

从源码构建:

```console
git clone https://github.com/can2049/qrtxt
cd qrtxt
cargo build --release
# 产物在 target/release/qrtxt
```

或从本地目录用 Cargo 安装:

```console
cargo install --path .
```

需要 Rust 1.85 或更高版本。

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

## 用法

```text
$ qrtxt -h
在终端把文字变成二维码:QR 码、Data Matrix、PDF417 或 Aztec Code。

Usage: qrtxt [COMMAND] [OPTIONS] [DATA]

命令:
  qr          生成普通二维码
  datamatrix  生成 Data Matrix(ECC 200)符号 [别名: dm]
  pdf417      生成 PDF417 符号 [别名: pdf]
  aztec       生成 Aztec Code 符号 [别名: az]
  help        打印帮助或子命令的帮助

选项:
  -h, --help     打印帮助(用 '--help' 查看更详细说明)
  -V, --version  打印版本

输入:
  -f, --file <PATH>       从文件读取 payload
  -p, --preserve-newline  保留文件/管道输入尾部的一个换行
  [DATA]                  字面量 payload

编码:
  -m, --max-bytes <BYTES>  限制每个符号的负载为 BYTES 字节;会触发拆分(默认: 单个符号自身的上限)
  -c, --chunk <COUNT>      把负载拆分成至少 COUNT 个符号;引导性(默认: 无下限)

渲染:
  -M, --mode <MODE>      绘制方式 [默认: auto] [可选值: auto, text, ansi, bitmap, kitty, sixel]
  -b, --border <BORDER>  静默区宽度(模块数);0 表示去掉留白 [默认: 4]
  -i, --invert           反转颜色:绘制浅底深字的反相二维码

文本渲染:
  -g, --glyphs <SET>  字形集,决定每个字符格打包多少模块 [默认: half] [可选值: half, quadrant, braille]

不带 COMMAND 时生成 QR 码,即 `qrtxt DATA` 等价于 `qrtxt qr DATA`。选项写在子命令之后。

示例:
  qrtxt "https://example.com"   QR 码(默认)
  cat token.txt | qrtxt         编码管道输入
  qrtxt datamatrix "hi"         Data Matrix 符号
  qrtxt pdf417 -C 1 "hi"        窄而高的 PDF417
  qrtxt aztec --layers -3 "hi"  紧凑型 Aztec Code
  qrtxt --mode text "hi"        强制使用 Unicode 块字符
  qrtxt --mode bitmap "hi"      强制位图(Kitty 或 Sixel)

Source: https://github.com/can2049/qrtxt
```

不带子命令时按 `qr` 处理,因此 `qrtxt "hi"` 与 `cat token.txt | qrtxt` 依旧可用。
共享选项在 `qrtxt -h` 与各子命令下都会列出;`-e/--error-correction` 按格式区分
(`qr` 取 `L`/`M`/`Q`/`H`,`pdf417` 取 `0`–`8`,`aztec` 取百分比 `0`–`100`),
运行 `qrtxt qr --help`、`qrtxt datamatrix --help`、`qrtxt pdf417 --help` 或
`qrtxt aztec --help` 查看对应格式的选项,`qrtxt qr -h` 查看简要摘要。

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

## 示例

```console
# 字面量字符串(不带子命令时默认按 qr 处理)
qrtxt "https://example.com"

# 等价的显式写法
qrtxt qr "https://example.com"

# 把密钥直接喷到屏幕,全程不落盘
cat token.txt | qrtxt

# 从文件读取
qrtxt qr --file payload.txt

# 输出 Data Matrix(ECC 200)符号,而非 QR 码
qrtxt datamatrix "https://example.com"

# 输出堆叠式 PDF417 符号
qrtxt pdf417 "https://example.com"

# 更高的 PDF417 纠错等级(0-8)
qrtxt pdf417 --error-correction 6 "important payload"

# 更窄更高的 PDF417(适配窄终端)
qrtxt pdf417 -C 1 "https://example.com"

# 同上,并把打印宽度减半
qrtxt pdf417 --cl 1 --mode text --glyphs braille "https://example.com"

# 输出 Aztec Code 符号
qrtxt aztec "https://example.com"

# 更高纠错、固定层数的 Aztec Code
qrtxt aztec --error-correction 60 --layers 5 "important payload"

# 紧凑型 Aztec Code(层数为负)
qrtxt aztec --layers -3 "https://example.com"

# 清晰的黑白位图(需要支持 Kitty 协议的终端)
qrtxt qr --mode kitty "https://example.com"

# 清晰的黑白位图(需要支持 Sixel 协议的终端)
qrtxt qr --mode sixel "https://example.com"

# 位图:终端支持哪种就用哪种(Kitty 或 Sixel)
qrtxt qr --mode bitmap "https://example.com"

# 强制使用 Unicode 块字符(用于管道,或与位图对照)
qrtxt qr --mode text "hello"

# 不支持块字符的终端:用 ANSI 转义序列
qrtxt qr --mode ansi "hello"

# 更高的纠错等级(仅 qr)
qrtxt qr --error-correction H "important payload"

# 更窄静默区
qrtxt qr --border 1 "hello"

# 为窄终端选择更高密度
qrtxt qr --mode text --glyphs braille "hello"

# 反相:浅底深字,替代默认的白底黑字
qrtxt qr --invert "hello"

# 自动拆分,每个二维码最多 500 字节
qrtxt qr --max-bytes 500 --file big.txt

# 拆分成至少 4 个负载均衡的二维码
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

## 条码格式

子命令决定生成哪种符号,`--mode`(见下)决定如何绘制。所有格式都支持全部渲染模式。

| 子命令 | 符号 |
|---|---|
| `qr`(默认) | 普通二维码;`--error-correction` 取 `L`/`M`/`Q`/`H`。 |
| `datamatrix`(`dm`) | Data Matrix(ECC 200)符号,始终为正方形。 |
| `pdf417`(`pdf`) | 堆叠式 PDF417 符号;`--error-correction` 取 `0`–`8`。 |
| `aztec`(`az`) | Aztec Code 符号;`--error-correction` 取 `0`–`100`,`--layers` 固定形状。 |

不带子命令时按 `qr` 处理,因此 `qrtxt "hi"` 与 `cat token.txt | qrtxt` 依旧可用。
Data Matrix 的纠错由符号尺寸固定,因此 `datamatrix` 没有任何编码选项,并会拒绝
`--error-correction`。

Data Matrix 单个符号能容纳的数据远少于 QR 码(最低纠错等级下约 1555 字节,QR 约
2953 字节),因此相同内容会拆成更多符号。PDF417 容量更小——默认等级下约 1068 字节,
等级 0 约 1075 字节——且其纠错等级与 QR 相反:从 `0` 到 `8` 每升一级,纠错码字数
大致翻倍,等级越高越健壮但容量越小。非 ASCII 文本同样声明 UTF-8 ECI;与 QR 一样,
不少扫描器对此处理不一致,能只用可打印 ASCII 时就尽量只用。任何无法表示为 UTF-8
文本的负载(例如原始二进制)会经 Latin-1 映射编码,从而原样还原字节。

PDF417 符号是一摞行,宽度与高度此消彼长:默认由编码器挑一个均衡的形状,偏宽而扁。
用 `--columns N`(1–30)可强制数据列数——符号宽 `17 × N + 69` 个模块,`N` 越小越高。
`--columns 1` 得到最窄最高的符号,数值越大越宽越扁;也接受缩写 `-C` 与 `--cl`。
若只为适配终端宽度,
`--mode text --glyphs quadrant`(或 `braille`)也能把每格塞进两个模块,把打印宽度大致减半。

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

## 渲染模式

`--mode` 决定二维码如何绘制:

| `--mode` | 家族 | 绘制内容 |
|---|---|---|
| `auto`(默认) | — | 终端支持时用位图,否则用 Unicode 块字符。 |
| `text` | 字符 | Unicode 块字符(字形集生效)。 |
| `ansi` | 字符 | 白底黑字的转义序列;更宽,用于不支持块字符的终端。 |
| `bitmap` | 位图 | 终端支持哪种协议就用哪种(Kitty 或 Sixel)绘制位图。 |
| `kitty` | 位图 | 通过 Kitty 图形协议绘制黑白位图。 |
| `sixel` | 位图 | 通过 Sixel 图形协议绘制同样的位图。 |

位图比块字符更小更清晰,因此 `auto` 优先使用它——但仅在合适的时候。标准输出不是
终端时 `auto` **绝不**输出位图,所以 `qrtxt ... > code.txt` 或 `qrtxt ... | head`
始终得到文本。`--mode bitmap` 强制位图,并依据终端支持在 Kitty/Sixel 中择优;
`--mode kitty` 与 `--mode sixel` 强制使用对应协议。三者在终端画不出来时都以用法
错误(退出码 2)退出。

`--glyphs` 仅对 `text` 生效;`--invert` 对所有模式生效(位图模式下交换两种颜色)。
`--help` 会对当前终端探测一次,并在 `--mode` 说明末尾附上 `auto` 会选哪一种。

## Glyph sets

| `--glyphs` | Modules per cell | Notes |
|---|---|---|
| `half` (h)     | 1 × 2        | Default. Most robust across fonts. |
| `quadrant` (q) | 2 × 2        | Twice the horizontal density. |
| `braille` (b)  | 2 × 4        | Densest; depends on the font rendering dots tightly. |

Each glyph set also accepts its first letter (`-g h`, `-g q`, `-g b`).

## 字形集

| `--glyphs` | 每字符格的模块数 | 说明 |
|---|---|---|
| `half` (h)     | 1 × 2        | 默认。跨字体最稳。 |
| `quadrant` (q) | 2 × 2        | 横向密度翻倍。 |
| `braille` (b)  | 2 × 4        | 密度最高;依赖字体把点渲染得足够密。 |

每个字形集也接受其首字母(`-g h`、`-g q`、`-g b`)。仅用于 `--mode text`;位图与
ANSI 渲染会忽略它。

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

## 深色与浅色终端

字符渲染器(`text` 与 `ansi`)会显式设置颜色:亮区(浅色模块与静默区)为白色、深色模块
为黑色。因此二维码是一张自洽的白底黑字图像,不论终端自身的背景色与配色如何都能扫——
包括**黑色背景**终端:若渲染器沿用终端颜色,静默区在黑色背景上会变成黑色。加
`--invert` 可交换两种颜色,得到浅底深字的反相结果。位图模式同样自带白色静默区。`qrtxt`
不会去探测终端背景色。这些颜色转义是文本输出的一部分,因此管道或重定向运行 `text`/`ansi`
也会带上它们——需要纯字形时请自行剥除。

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

## Kitty 图形协议

`--mode kitty` 改用 [Kitty 图形协议](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
把二维码绘制成真正的位图,不再受字体如何渲染块字符的限制,因此画得更小。它需要终端实现该协议
——例如 kitty、Ghostty、WezTerm。`qrtxt` 会先用 `a=q` 握手探测终端;若协议不受支持
(或标准输出不是终端),则直接以用法错误退出,而不是输出终端无法渲染的转义字节。
输出为纯黑白,并自带白色静默区,因此 `--invert` 会交换两种颜色而二维码保持自洽;
字形集选项被忽略。

`auto` 与 `bitmap` 也用同一次探测在 Kitty 与 Sixel 之间优先选 Kitty;`--help` 探测
会在 `--mode` 说明末尾附上 `auto` 对当前终端会选哪一种。

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

## Sixel 图形协议

`--mode sixel` 绘制与 `--mode kitty` 相同的黑白位图,但走较老的
[Sixel 图形格式](https://en.wikipedia.org/wiki/Sixel)。两者覆盖不同的终端:Sixel 触达
xterm、Konsole、foot、mlterm、iTerm2、Windows Terminal(1.22+)——这些终端不实现
Kitty 协议;而 kitty、Ghostty 实现 Kitty 却不支持 Sixel。由于二维码只有两种颜色,
Sixel 输出仅用两个调色板寄存器 + 游程编码,因此非常紧凑。

Sixel 的支持探测比 Kitty 弱:它没有握手,`qrtxt` 会发送一次主设备属性查询(DA1),
只有广告支持 Sixel(设备属性 `4`)的终端才被接受;支持但不广告的终端按"不支持"处理,
因此绝不会输出终端画不出来的转义字节。与 `--mode kitty` 一样,输出为纯黑白并自带白色
静默区,`--invert` 交换两种颜色,字形集选项被忽略;`--help` 探测同样在 `--mode` 说明
末尾给出结论。

图像会声明方形像素,因此在遵守光栅属性的终端上模块保持方正——但 Sixel 渲染尚未在所有
终端上目视验证,请确认打印出来的二维码仍可扫。

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

## 超长内容

单个符号能容纳的数据有上限(`L` 级二维码约 2953 字节,级别越高越少)。放不下时,`qrtxt`
会自动把内容均分到多个符号(无需任何参数),按顺序输出,每个符号上方带 `QR Code i/N`
(或 `Data Matrix i/N`、`PDF417 i/N`)说明。各符号负载均衡,承载的数据量彼此相近;
拆分文本时切点落在字符边界上,因此多字节字符不会被拆到两个符号里。能放进单个符号的
内容仍作为单个符号输出、不带说明。

用 `--max-bytes BYTES` 可限制每个二维码的负载字节数(便于生成更小、更易扫的二维码)。
该选项会触发拆分:即使整体能放进一个二维码,也会拆开以保证每个二维码不超过上限。
上限若超过单个符号自身能容纳的量,会被自动收敛到该符号的上限。

用 `--chunk COUNT` 可把负载尽量分散到**至少** COUNT 个二维码(例如想让每个二维码都
更小,或按页面排版)。它是引导性参数:内容允许时拆成 COUNT 个负载均衡的二维码,否则
拆成尽可能多(上限为每个字符一个二维码)。`--chunk` 与 `--max-bytes` 可同时使用,谁要求
的二维码更多就以谁为准。

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `2` | Usage or input error (empty input, conflicting flags) |
| `1` | Runtime/IO error (unreadable file, write failure) |

A closed downstream pipe (for example `qrtxt ... | head`) is treated as success.

## 退出码

| 码 | 含义 |
|---|---|
| `0` | 成功 |
| `2` | 用法/输入错误(空输入、参数冲突) |
| `1` | 运行时/IO 错误(文件不可读、写入失败) |

下游管道提前关闭(例如 `qrtxt ... | head`)视为成功。

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

## 实现说明

```text
argv / stdin -> input::resolve -> encode::encode_multi -> render::build_ink -> render::render_on_field -> stdout
```

代码分层,依赖单向:

- `cli` 负责参数解析(`parse`)与流程编排(`run` / `run_with`)。
- `input`、`encode`、`render` 是纯逻辑,不依赖 `clap`。
- `encode` 按符号类型包装对应的条码库(QR 用 `qrcode`,Data Matrix 用 `datamatrix`,
  PDF417 用 `rxing`),
  并产出与符号类型无关的 `Symbol`(明暗模块网格),因此只有 `encode` 依赖条码库。
- `kitty` 与 `sixel` 存放底层协议封帧,供 `cli`(终端探测)与 `render`(位图传输)共用。
- `types` 存放共享值类型(含 `Symbol`);`error` 把失败映射为退出码。

渲染分两步:`build_ink` 把 `Symbol` 转成布尔“墨色”网格(施加静默区、反相),
`render` 再按字形集把网格打包成字符;`render_on_field` 给这些字符套上显式颜色,使深色
模块打印成白底黑字,`render_ansi` 用转义序列绘制同一网格。`render_kitty` 与
`render_sixel` 跳过该网格,直接通过 Kitty 或 Sixel 协议输出黑白位图。`render` 自身按策略
拆成 `frame`/`text`/`ansi`/`bitmap` 子模块,由 `render_all` 统一分发。round-trip 测试会
先剥掉颜色转义、把输出还原成模块网格、再解码——QR 用 `rqrr`,Data Matrix 用
`datamatrix` crate,PDF417 用 `rxing`——以证明打印出来的东西仍然可扫。

## License

MIT. See [LICENSE](LICENSE).

## 许可

MIT,见 [LICENSE](LICENSE)。
