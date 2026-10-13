# Data Matrix(ECC 200)输出 · 设计说明

> `qrtxt --format datamatrix` 输出 **Data Matrix(ECC 200)** 二维条码,与 QR 码并列。
> 状态:已实现并提交(分支 `data-matrix`),测试全绿。
>
> **后续变更**:`--format` 选择器已被 `qr` / `datamatrix` 子命令取代
> (`qrtxt datamatrix ...`)。本文保留当时的 `--format` 表述作为该次改动的历史记录,
> 当前的命令行形态以 [`README.md`](../README.md) 为准。

**动机**:调研「能否支持 Data Matrix 输出」。结论是可行,但需要先把与 `qrcode` 紧耦合的
渲染层解耦,再接入第二种符号。本文记录调研 → 架构 → 实现 → 验证的完整设计过程。

---

## 一、调研与选型

### 结论

技术可行。Data Matrix 与 QR 同属「明暗模块矩阵」,现有五种渲染模式(text/ansi/kitty/sixel)
全部可复用;真正的成本在**符号抽象**与**分片泛化**,而不在渲染。

### 库选型

| 库 | 许可 | 说明 |
|---|---|---|
| **`datamatrix`**(jannschu/datamatrix-rs)v0.3.3 | Apache-2.0 OR MIT | 专一实现 ECC200,**优化编码器 + 内置解码器**,`no_std`;`encode` 返回 `Bitmap<bool>`,可用 `width()`/`height()`/`pixels()`。默认特征为空,可选 `encoding_rs`(扩展 ECI)。 |
| `rxing`(ZXing 移植) | Apache-2.0 | 编解码覆盖多符号(QR/Aztec/PDF417…),但依赖重,且其文档自述 Data Matrix 解码存在已知毛病。 |

**选择 `datamatrix`**:专一、返回位图、许可与项目 MIT 兼容,且**自带解码器**正好补上
`rqrr`(只能解 QR)的空缺,可直接用于 round-trip 验证。不自己手写 ECC200(Reed-Solomon +
放置规则复杂,风险高)。

### 与 QR 的关键差异

| 维度 | QR | Data Matrix(ECC200) |
|---|---|---|
| 纠错等级 | L/M/Q/H **可选** | **随符号尺寸固定**,无用户选项 |
| 最大容量 | v40-L ≈ 2953 字节(字节模式) | 144×144 ≈ 1558 字节(base256);数字模式 3116 |
| 形状 | 恒为正方形 | 正方形 / 矩形(本项目**仅正方形**) |
| 默认字符集 | 无内建(靠 ECI 或解码器猜测) | ISO-8859-1(拉丁-1) |

---

## 二、技术架构与流程

### 数据流

```
input -> encode::{Codec} -> render -> stdout
```

`encode` 按 `--format` 选择 codec,**产出与符号类型无关的 `types::Symbol`**;`render` 完全复用。

### 解耦:中立的 `Symbol`

先做一步纯重构(不改变行为):**`src/types.rs` 新增 `Symbol`**——`width`/`height` 与
`is_dark(x, y)` 组成的明暗网格。`encode` 产出它,`render`/`cli` 消费它,于是 `render/`
与 `cli/mod.rs` **不再依赖 `qrcode`**(仅 `encode.rs` 依赖条码库)。位图路径同时从「方形」
推广到任意 W×H,为将来的矩形符号留门。

### 编码器抽象:`Codec`

`src/encode.rs` 定义 trait:

```rust
trait Codec {
    fn encode(&self, data: &[u8]) -> Result<Symbol, AppError>;
    fn fits(&self, data: &[u8]) -> bool;
    fn capacity(&self) -> usize; // 分片二分搜索的上界
}
```

- `QrCodec { ec }`:`qrcode` 包装(保持原有精确纠错与 UTF-8 ECI 处理)。
- `DataMatrixCodec`:`DataMatrix::encode_str`(合法 UTF-8,自动决定拉丁-1 或 UTF-8 ECI)/
  `encode`(其余字节),用 `SymbolList::default().enforce_square()` 限定方形。纠错等级**不参与**。

**分片算法只写一份**:`split_chunks_with(codec, …)` 基于 `codec.fits` / `codec.capacity`
工作,「均衡切分、按 UTF-8 字符边界回退、贪心兜底」等逻辑对两种符号通用,`--max-bytes` /
`--chunk` 因此对 QR 与 DM 一并生效。

### CLI:`--format` 与 `--mode` 正交

- 新开关 `-F/--format qr|datamatrix`(`dm` 简写),新增 `types::Format`。
- `--mode` 仍是**渲染**选择器;`--format` 是**符号**选择器,二者互不影响,五种渲染模式都对
  DM 生效。
- 多符号标题按格式取 `QR Code i/N` / `Data Matrix i/N`(`render` 的 `format_label`)。

---

## 三、关键决策与取舍

| 决策 | 取值 | 理由 |
|---|---|---|
| 符号形状 | **仅正方形** | 终端里更规整,也避免矩形带来的额外几何与测试复杂度 |
| `--error-correction` | 对 DM **忽略**并文档标注「仅 QR」 | ECC200 纠错随符号尺寸固定,无用户可选等级;`-e` 有默认值,命令行难以区分「是否用户显式传入」 |
| 非 ASCII | 复用 UTF-8 ECI | 与 QR 一致;`encode_str` 仅在超出拉丁-1 时插入 ECI |
| 静默区 | 沿用 `--border`(默认 4) | DM 规范仅要求 ≥1 模块;4 更大但无害 |
| 兼容层 | 无 | 新特性,不引入开关或回退分支 |

---

## 四、验证

- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings` 通过。
- `cargo test`:lib + cli + roundtrip 全绿。
- **可扫性(核心)**:
  - `datamatrix_ascii_round_trips`:ASCII 负载走完整「CLI 渲染 → 解析网格 →
    `DataMatrix::decode` 解码」往返(多字形集)。
  - `datamatrix_cjk_round_trips`:**CJK/emoji** 负载——crate 的像素解码器拒绝带 ECI 的符号,
    故以两段合成证明:① `data::decode_str(codewords)` 解回原文;② 我方 `encode` 产出的符号与
    crate 符号**逐模块相等**。二者叠加即证明打印出的 CJK 符号可解回原文。
  - `datamatrix_grid_matches_the_encoder`:全部 `CJK_SAMPLES` × 3 组字形/反相,逐模块比对打印
    网格与编码器符号网格(证明渲染无损)。
  - `datamatrix_split_round_trips_in_order` / `datamatrix_max_bytes_split_round_trips_in_order`:
    自然拆分与 `--max-bytes` 拆分,逐块解码、按序拼接还原。
  - `datamatrix_symbol_sizes_round_trip`:最小符号(1 字节)与较大符号。
  - `datamatrix_inverted_round_trips`:反相可扫。
- **单测**:`encode` 侧方形校验 / 非 ASCII 可编码 / 超限报错 / 分片覆盖。
- **行为**:`tests/cli.rs` 覆盖 `-F dm` 与 `--format datamatrix` 等价、多码 `Data Matrix i/N` 标题、
  未知格式退出码 2、`-h` 广告 `dm` 简写。

---

## 五、参考

### 未决

- **需求编号**:代码注释中的 `FR-2.4`–`FR-2.7` 为占位符(外部 spec 不在本仓库),需核对替换。
- **纠错开关语义**:对 DM 是「静默忽略」还是「显式传入即报用法错误」,待定。
- 尚未 push / 未建 PR,版本未 bump。

### 链接

- `datamatrix` crate:<https://crates.io/crates/datamatrix> · <https://docs.rs/datamatrix>
- 源码:<https://github.com/jannschu/datamatrix-rs/>
- `rxing`(备选):<https://github.com/rxing-core/rxing>
- Data Matrix(ECC200)规范综述:<https://www.tec-it.com/en/support/knowbase/symbologies/datamatrix/Default.aspx>
- 同源文档:[`kitty-graphics-protocol.md`](./kitty-graphics-protocol.md) ·
  [`sixel-graphics-protocol.md`](./sixel-graphics-protocol.md)
