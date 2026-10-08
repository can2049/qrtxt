# qrterm

在终端里把文字变成二维码。

[English](README.md) | **简体中文**

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)

`qrterm` 读取字符串、文件或管道输入,用 Unicode 块字符在标准输出上打印一个普通
二维码。无需图形环境,只要有 UTF-8 终端即可。

```console
$ qrterm "hi"

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
- **默认安全** —— 无 `unsafe` 代码,错误信息绝不回显 payload。
- **单一静态二进制** —— 无运行时依赖。

## 安装

从源码构建:

```console
git clone https://github.com/can2049/qrterm
cd qrterm
cargo build --release
# 产物在 target/release/qrterm
```

或用 Cargo 直接安装:

```console
cargo install --path .
```

需要 Rust 1.85 或更高版本。

## 用法

```text
qrterm [OPTIONS] [DATA]

参数:
  [DATA]                 字面量 payload;省略时从 --file 或 stdin 读取

选项:
  -f, --file <PATH>      从文件读取 payload
      --preserve-newline 不剥除管道/文件输入尾部的单个换行
  -e, --error <LEVEL>    精确纠错等级: L、M、Q 或 H [默认: M]
  -b, --border <N>       静默区宽度(模块数)          [默认: 4]
  -s, --scale <N>        终端模块缩放倍数            [默认: 1]
      --invert           反转墨色映射,用于浅色背景终端
      --glyphs <SET>     字形集: half、quadrant 或 braille [默认: half]
      --no-compact       使用 ANSI 渲染替代 Unicode 块字符
  -h, --help             打印帮助
  -V, --version          打印版本
```

## 示例

```console
# 字面量字符串
qrterm "https://example.com"

# 把密钥直接喷到屏幕,全程不落盘
cat token.txt | qrterm

# 从文件读取
qrterm --file payload.txt

# 更高的纠错等级
qrterm --error H "important payload"

# 更大尺寸、更窄静默区
qrterm --scale 2 --border 1 "hello"

# 为窄终端选择更高密度
qrterm --glyphs braille "hello"

# 浅色背景终端
qrterm --invert "hello"
```

## 字形集

| `--glyphs` | 每字符格的模块数 | 说明 |
|---|---|---|
| `half`     | 1 × 2            | 默认。跨字体最稳。 |
| `quadrant` | 2 × 2            | 横向密度翻倍。 |
| `braille`  | 2 × 4            | 密度最高;依赖字体把点渲染得足够密。 |

`--scale`(物理大小)与 `--glyphs`(逻辑密度)是两个相互独立的开关。

## 深色与浅色终端

块字符以终端**前景色**绘制,因此默认输出假设深色背景终端。浅色背景终端下二维码
会整体反相,此时加 `--invert` 可恢复可扫的 dark-on-light 结果。`qrterm` 不会去探测
终端背景色。

## 退出码

| 码 | 含义 |
|---|---|
| `0` | 成功 |
| `2` | 用法/输入错误(空输入、参数冲突、数据过长) |
| `1` | 运行时/IO 错误(文件不可读、写入失败) |

下游管道提前关闭(例如 `qrterm ... | head`)视为成功。

## 实现说明

```text
argv / stdin -> input::resolve -> encode::encode -> render::build_ink -> render::render -> stdout
```

代码分层,依赖单向:

- `cli` 负责参数解析与流程编排(`run` / `run_with`)。
- `input`、`encode`、`render` 是纯逻辑,不依赖 `clap`。
- `types` 存放共享值类型;`error` 把失败映射为退出码。

渲染分两步:`build_ink` 把二维码矩阵转成布尔“墨色”网格(施加静默区、缩放、反相),
`render` 再按字形集把网格打包成字符。round-trip 测试会先把输出还原成模块网格、再用
`rqrr` 解码,以证明打印出来的东西仍然可扫。

## 许可

MIT,见 [LICENSE](LICENSE)。
