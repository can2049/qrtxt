# qrtxt

在终端里把文字变成二维码。

[English](README.md) | **简体中文**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`qrtxt` 读取字符串、文件或管道输入,用 Unicode 块字符在标准输出上打印一个普通
二维码。无需图形环境,只要有 UTF-8 终端即可。

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

(静默区渲染为空白,所以二维码看起来是“悬浮”的。)

## 特性

- **三种输入方式** —— 位置参数字面量、文件(`--file`)、标准输入。
- **精确纠错等级** —— `L`、`M`、`Q`、`H`,不做自动升档。
- **三种字形集** —— 半块(默认)、象限块、盲文点阵,在可扫性与屏幕密度之间取舍。
- **ANSI 渲染** —— `--no-compact`,用于不支持块字符的终端。
- **深色/浅色终端** —— `--invert` 适配浅色背景。
- **超长内容** —— 放不进单个二维码的内容会自动均分成多个二维码;`--max-size` 可限制每个二维码的字节数,`--chunk` 可指定二维码数量的下限。
- **默认安全** —— 无 `unsafe` 代码,错误信息绝不回显 payload。
- **单一静态二进制** —— 无运行时依赖。

## 安装

从源码构建:

```console
git clone https://github.com/can2049/qrtxt
cd qrtxt
cargo build --release
# 产物在 target/release/qrtxt
```

或用 Cargo 直接安装:

```console
cargo install --path .
```

需要 Rust 1.85 或更高版本。

## 用法

```text
qrtxt [OPTIONS] [DATA]

参数:
  [DATA]                 字面量 payload;省略时从 --file 或 stdin 读取

选项:
  -g, --glyphs <SET>             字形集,决定每个字符格打包多少模块: half (h;1x2 模块/格,最稳)、quadrant (q;2x2)、braille (b;2x4,最密) [默认: half]
  -e, --error-correction <LEVEL> 纠错等级: L(约可恢复 7%)、M(15%)、Q(25%)、H(30%) [默认: L]
  -m, --max-size <BYTES>         限制每个二维码的负载为 BYTES 字节;会触发拆分(默认: 单个符号自身的上限)
  -c, --chunk <COUNT>            把负载拆分成至少 COUNT 个二维码;引导性(默认: 无下限)
  -a, --no-compact               用 ANSI 转义序列渲染,替代 Unicode 块字符
  -f, --file <PATH>              从文件读取 payload
  -p, --preserve-newline         保留文件/管道输入尾部的一个换行
  -b, --border <BORDER>          静默区宽度(模块数);0 表示去掉留白 [默认: 4]
  -i, --invert                   反转墨色,用于浅色背景终端
  -h, --help                     打印帮助(用 '--help' 查看更详细说明)
  -V, --version                  打印版本
```

运行 `qrtxt --help` 可查看每个选项的完整说明及其参数的作用(`-h` 只打印简要摘要)。

## 示例

```console
# 字面量字符串
qrtxt "https://example.com"

# 把密钥直接喷到屏幕,全程不落盘
cat token.txt | qrtxt

# 从文件读取
qrtxt --file payload.txt

# 更高的纠错等级
qrtxt --error-correction H "important payload"

# 更窄静默区
qrtxt --border 1 "hello"

# 为窄终端选择更高密度
qrtxt --glyphs braille "hello"

# 浅色背景终端
qrtxt --invert "hello"

# 自动拆分,每个二维码最多 500 字节
qrtxt --max-size 500 --file big.txt

# 拆分成至少 4 个负载均衡的二维码
qrtxt --chunk 4 "a-short-but-verifiable-payload"
```

## 字形集

| `--glyphs` | 每字符格的模块数 | 说明 |
|---|---|---|
| `half` (h)     | 1 × 2        | 默认。跨字体最稳。 |
| `quadrant` (q) | 2 × 2        | 横向密度翻倍。 |
| `braille` (b)  | 2 × 4        | 密度最高;依赖字体把点渲染得足够密。 |

每个字形集也接受其首字母(`-g h`、`-g q`、`-g b`)。

## 深色与浅色终端

块字符以终端**前景色**绘制,因此默认输出假设深色背景终端。浅色背景终端下二维码
会整体反相,此时加 `--invert` 可恢复可扫的 dark-on-light 结果。`qrtxt` 不会去探测
终端背景色。

## 超长内容

单个二维码能容纳的数据有上限(`L` 级约 2953 字节,级别越高越少)。放不下时,`qrtxt`
会自动把内容均分到多个二维码(无需任何参数),按顺序输出,每个二维码上方带 `QR i/N`
说明。各二维码负载均衡,承载的数据量彼此相近;拆分文本时切点落在字符边界上,因此
多字节字符不会被拆到两个二维码里。能放进单个二维码的内容仍作为单个二维码输出、
不带说明。

用 `--max-size BYTES` 可限制每个二维码的负载字节数(便于生成更小、更易扫的二维码)。
该选项会触发拆分:即使整体能放进一个二维码,也会拆开以保证每个二维码不超过上限。
上限若超过单个符号自身能容纳的量,会被自动收敛到该符号的上限。

用 `--chunk COUNT` 可把负载尽量分散到**至少** COUNT 个二维码(例如想让每个二维码都
更小,或按页面排版)。它是引导性参数:内容允许时拆成 COUNT 个负载均衡的二维码,否则
拆成尽可能多(上限为每个字符一个二维码)。`--chunk` 与 `--max-size` 可同时使用,谁要求
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
argv / stdin -> input::resolve -> encode::encode_multi -> render::build_ink -> render::render -> stdout
```

代码分层,依赖单向:

- `cli` 负责参数解析与流程编排(`run` / `run_with`)。
- `input`、`encode`、`render` 是纯逻辑,不依赖 `clap`。
- `types` 存放共享值类型;`error` 把失败映射为退出码。

渲染分两步:`build_ink` 把二维码矩阵转成布尔“墨色”网格(施加静默区、反相),
`render` 再按字形集把网格打包成字符。round-trip 测试会先把输出还原成模块网格、再用
`rqrr` 解码,以证明打印出来的东西仍然可扫。

## 许可

MIT,见 [LICENSE](LICENSE)。
