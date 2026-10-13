# qrtxt

在终端里把文字变成二维码。

[English](README.md) | **简体中文**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![crates.io](https://img.shields.io/crates/v/qrtxt.svg)](https://crates.io/crates/qrtxt)

`qrtxt` 读取字符串、文件或管道输入,在标准输出上打印一个普通二维码;用
`datamatrix` 子命令则打印 Data Matrix(ECC 200)符号,用 `pdf417` 子命令则打印堆叠式
PDF417 符号。默认它会挑选终端所能提供
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

(字符渲染器会显式设置颜色:深色模块在黑字白底上打印、静默区保持白色,因此不受终端
自身背景色影响;上面这段无色转写里静默区显示为空白。)

## 特性

- **三种输入方式** —— 位置参数字面量、文件(`--file`)、标准输入。
- **三种条码格式** —— 各对应一个子命令:`qr`(普通二维码)、`datamatrix`(Data Matrix(ECC 200)符号)与 `pdf417`(堆叠式 PDF417 符号)。
- **按格式区分的纠错等级** —— `qr` 取 `L`、`M`、`Q`、`H`(不做自动升档),`pdf417` 取其数字等级 `0`–`8`;Data Matrix 的纠错由符号尺寸固定,因此 `datamatrix` 没有该选项。
- **五种渲染模式** —— `--mode auto`(默认)挑选终端的最佳渲染方式:支持时用
  Kitty/Sixel 图形协议绘制清晰位图,否则回退到 Unicode 块字符;也可用 `text`、
  `ansi`、`kitty`、`sixel` 强制指定。位图比块字符更小更清晰,`ansi` 覆盖不支持块
  字符的终端,不受支持的位图模式会快速报错而不是打印乱码。
- **三种字形集** —— 半块(默认)、象限块、盲文点阵,在可扫性与屏幕密度之间取舍
  (仅用于 `--mode text`)。
- **任意终端都可读** —— 字符渲染器显式设置颜色,把深色模块打印成白底黑字(静默区为白色),
  因此在深色或浅色背景终端上都能扫;`--invert` 可换成浅底深字(反相)。
- **超长内容** —— 放不进单个二维码的内容会自动均分成多个二维码;`--max-bytes` 可限制每个二维码的字节数,`--chunk` 可指定二维码数量的下限。
- **默认安全** —— 无 `unsafe` 代码,错误信息绝不回显 payload。
- **单一静态二进制** —— 无运行时依赖。

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

## 用法

```text
qrtxt <COMMAND> [OPTIONS] [DATA]

命令:
  qr          生成普通二维码
  datamatrix  生成 Data Matrix(ECC 200)符号 [别名: dm]
  pdf417      生成 PDF417 符号 [别名: pdf]
  help        打印帮助或子命令的帮助

选项:
  -h, --help     打印帮助(用 '--help' 查看更详细说明)
  -V, --version  打印版本

下列选项为所有子命令共享;`qr` 与 `pdf417` 各自额外提供 -e/--error-correction。

输入:
  -f, --file <PATH>       从文件读取 payload
  -p, --preserve-newline  保留文件/管道输入尾部的一个换行
  [DATA]                  字面量 payload;省略时从 --file 或 stdin 读取

编码:
  -e, --error-correction <LEVEL>  (qr) 纠错等级: L(约可恢复 7%)、M(15%)、Q(25%)、H(30%) [默认: L]
                                  (pdf417) 纠错等级: 0(冗余最少)到 8(最多) [默认: 2]
  -m, --max-bytes <BYTES>         限制每个符号的负载为 BYTES 字节;会触发拆分(默认: 单个符号自身的上限)
  -c, --chunk <COUNT>             把负载拆分成至少 COUNT 个符号;引导性(默认: 无下限)

渲染:
  -M, --mode <MODE>      绘制方式: auto、text、ansi、kitty 或 sixel [默认: auto]
  -b, --border <BORDER>  静默区宽度(模块数);0 表示去掉留白 [默认: 4]
  -i, --invert           反转颜色:绘制浅底深字的反相二维码

文本渲染:
  -g, --glyphs <SET>  字形集,决定每个字符格打包多少模块: half (h;1x2 模块/格,最稳)、quadrant (q;2x2)、braille (b;2x4,最密) [默认: half]

Source: https://github.com/can2049/qrtxt
```

不带子命令时按 `qr` 处理,因此 `qrtxt "hi"` 与 `cat token.txt | qrtxt` 依旧可用。
运行 `qrtxt --help` 查看命令列表,`qrtxt qr --help`、`qrtxt datamatrix --help` 或
`qrtxt pdf417 --help` 查看各格式的选项,`qrtxt qr -h` 查看简要摘要。

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

# 清晰的黑白位图(需要支持 Kitty 协议的终端)
qrtxt qr --mode kitty "https://example.com"

# 清晰的黑白位图(需要支持 Sixel 协议的终端)
qrtxt qr --mode sixel "https://example.com"

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

## 条码格式

子命令决定生成哪种符号,`--mode`(见下)决定如何绘制。所有格式都支持全部渲染模式。

| 子命令 | 符号 |
|---|---|
| `qr`(默认) | 普通二维码;`--error-correction` 取 `L`/`M`/`Q`/`H`。 |
| `datamatrix`(`dm`) | Data Matrix(ECC 200)符号,始终为正方形。 |
| `pdf417`(`pdf`) | 堆叠式 PDF417 符号;`--error-correction` 取 `0`–`8`。 |

不带子命令时按 `qr` 处理,因此 `qrtxt "hi"` 与 `cat token.txt | qrtxt` 依旧可用。
Data Matrix 的纠错由符号尺寸固定,因此 `datamatrix` 没有任何编码选项,并会拒绝
`--error-correction`。

Data Matrix 单个符号能容纳的数据远少于 QR 码(最低纠错等级下约 1555 字节,QR 约
2953 字节),因此相同内容会拆成更多符号。PDF417 容量更小——默认等级下约 1068 字节,
等级 0 约 1075 字节——且其纠错等级与 QR 相反:从 `0` 到 `8` 每升一级,纠错码字数
大致翻倍,等级越高越健壮但容量越小。非 ASCII 文本同样声明 UTF-8 ECI;与 QR 一样,
不少扫描器对此处理不一致,能只用可打印 ASCII 时就尽量只用。任何无法表示为 UTF-8
文本的负载(例如原始二进制)会经 Latin-1 映射编码,从而原样还原字节。

## 渲染模式

`--mode` 决定二维码如何绘制:

| `--mode` | 家族 | 绘制内容 |
|---|---|---|
| `auto`(默认) | — | 终端支持时用位图,否则用 Unicode 块字符。 |
| `text` | 字符 | Unicode 块字符(字形集生效)。 |
| `ansi` | 字符 | 白底黑字的转义序列;更宽,用于不支持块字符的终端。 |
| `kitty` | 位图 | 通过 Kitty 图形协议绘制黑白位图。 |
| `sixel` | 位图 | 通过 Sixel 图形协议绘制同样的位图。 |

位图比块字符更小更清晰,因此 `auto` 优先使用它——但仅在合适的时候。标准输出不是
终端时 `auto` **绝不**输出位图,所以 `qrtxt ... > code.txt` 或 `qrtxt ... | head`
始终得到文本。`--mode kitty` 与 `--mode sixel` 强制使用对应协议,终端画不出来时
以用法错误(退出码 2)退出。

`--glyphs` 仅对 `text` 生效;`--invert` 对所有模式生效(位图模式下交换两种颜色)。
`--help` 会对当前终端探测一次,并在 `--mode` 说明末尾附上 `auto` 会选哪一种。

## 字形集

| `--glyphs` | 每字符格的模块数 | 说明 |
|---|---|---|
| `half` (h)     | 1 × 2        | 默认。跨字体最稳。 |
| `quadrant` (q) | 2 × 2        | 横向密度翻倍。 |
| `braille` (b)  | 2 × 4        | 密度最高;依赖字体把点渲染得足够密。 |

每个字形集也接受其首字母(`-g h`、`-g q`、`-g b`)。仅用于 `--mode text`;位图与
ANSI 渲染会忽略它。

## 深色与浅色终端

字符渲染器(`text` 与 `ansi`)会显式设置颜色:亮区(浅色模块与静默区)为白色、深色模块
为黑色。因此二维码是一张自洽的白底黑字图像,不论终端自身的背景色与配色如何都能扫——
包括**黑色背景**终端:若渲染器沿用终端颜色,静默区在黑色背景上会变成黑色。加
`--invert` 可交换两种颜色,得到浅底深字的反相结果。位图模式同样自带白色静默区。`qrtxt`
不会去探测终端背景色。这些颜色转义是文本输出的一部分,因此管道或重定向运行 `text`/`ansi`
也会带上它们——需要纯字形时请自行剥除。

## Kitty 图形协议

`--mode kitty` 改用 [Kitty 图形协议](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
把二维码绘制成真正的位图,不再受字体如何渲染块字符的限制,因此画得更小。它需要终端实现该协议
——例如 kitty、Ghostty、WezTerm。`qrtxt` 会先用 `a=q` 握手探测终端;若协议不受支持
(或标准输出不是终端),则直接以用法错误退出,而不是输出终端无法渲染的转义字节。
输出为纯黑白,并自带白色静默区,因此 `--invert` 会交换两种颜色而二维码保持自洽;
字形集选项被忽略。

`auto` 也用同一次探测在 Kitty 与 Sixel 之间优先选 Kitty;`--help` 探测会在 `--mode`
说明末尾附上 `auto` 对当前终端会选哪一种。

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

## 退出码

| 码 | 含义 |
|---|---|
| `0` | 成功 |
| `2` | 用法/输入错误(空输入、参数冲突) |
| `1` | 运行时/IO 错误(文件不可读、写入失败) |

下游管道提前关闭(例如 `qrtxt ... | head`)视为成功。

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

## 许可

MIT,见 [LICENSE](LICENSE)。
