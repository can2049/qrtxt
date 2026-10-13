# 架构评审 · 坏味道与耦合松绑建议

> 对 `qrtxt` 全量源码(`src/` 约 2900 行)的一次架构走查,记录发现的坏味道与可选的松绑方向。
> 状态:**高优先级项 #1、#2 已实施**(见文末「实施记录」);#3–#5 仍为待办建议。

**总体判断**:分层是干净的——`cli → {input, encode, render} → types/error` 单向依赖,
领域模块不依赖 `clap`,`#![forbid(unsafe_code)]` + 显式分层在 `lib.rs` 有清晰文档。
下述问题是「遵守了字面规则、但未完全遵守精神」的局部耦合,而非结构性缺陷。

---

## 一、渲染策略驻留在 CLI 层(影响最大)

`cli::render_all`(`src/cli/mod.rs:294-327`)承担了四种渲染器的分派,并携带了大量**渲染器内部知识**:

- `KITTY_IMAGE_ID_BASE` 与「每条码一个 image id」的方案(`:282`、`:303`)——这是 Kitty 协议细节。
- ANSI 下「把 frame 钉到 `GlyphSet::Half`」的绕过逻辑(`:317-319`)——它依赖 `GlyphSet::tile()`
  的语义(定义在 `src/render.rs:53` 的 `impl GlyphSet` 块),即 cli 伸手够到了 render 的不变量。
- 多码标题前缀 `format_label`(`:286`)——呈现层职责。

更明显的信号:两类渲染器入参不一致——kitty/sixel 收 `&Symbol`,text/ansi 收 `&Frame`,
于是 cli 必须知道每条路径各自走哪个「准备网格」的函数(`build_ink` vs 内部的 `module_bitmap`)。
测试也印证了这一点:kitty image-id、sixel 序列的断言都落在 `cli/mod.rs:511-707`,而非 `render`。

**松绑方向**:在 `render` 暴露单一入口,例如 `render::render(symbol, mode, opts, out)`,
把 mode 分派、image id 基址、ANSI 钉字形 都搬进去。cli 只负责组装 `Config` + IO,
正是 AGENTS.md 对 cli 的定位。

**权衡**:`render` 会新增对 `RenderMode`(一个 `types` 枚举,无妨)与多码版式的了解;
若认为标题纯属呈现,可留在 cli,仅搬分派逻辑。

## 二、`Config` 是上帝对象,却强加给 `input` 依赖

`input::resolve`(`src/input.rs:12-13`)收 `&Config`,实际只读 `cfg.input` 与 `cfg.preserve_newline`。
而 `Config`(`src/types.rs:208-231`)横跨输入、编码、渲染三类字段。结果是:任何**渲染字段**的改动,
都会把一条名义依赖牵进 `input`。

值得注意的是,`encode_multi` 与各渲染器都已是窄参数——`input` 是唯一的例外,却抱着整个聚合体。

**松绑方向**:改传 `&InputSpec` + `preserve_newline: bool`(或一个小的 `InputOptions`)。
`input` 便只依赖 `InputSpec`/`error`,不再依赖跨域 `Config`。风险低,改动小。

## 三、`Symbol` 与 `Frame` 机械上同构

`src/types.rs:151` 与 `src/render.rs:13` 都是 `{width, height, Vec<bool>}`:
同样的长度不变量断言、同样的 `width`/`height`、同样的单元访问器(`is_dark` vs `at`)。
`Frame` 本质就是「`Symbol` + 按字形 tile 补齐」。

**松绑方向(低优先)**:抽一个两者共用的泛型 `Grid` newtype,或让 `build_ink` 产出 `Symbol`。

**权衡**:两者语义不同(「条码模块矩阵」vs「可绘制的补齐网格」),合并有概念混淆风险;
除非出现第三种网格类型,否则不必动。

## 四、`encode.rs` 把通用分片器与 QR 专属逻辑混在一起

`Codec` trait 与 `split_chunks_with` / `even_chunks` / `greedy_chunks` / `max_prefix_fitting`
(`src/encode.rs:115-377`)是**符号无关**的,却与 QR 的 ECI/Kanji 逻辑挤在一个以 QR 命名的文件里。
抽象本身是对的,只是地址不对。

**松绑方向(低优先)**:待第三种符号落地时,抽成独立的 `split`(或 `codec`)模块。
当前两种格式下属过早优化。

## 五、命名与构造的小瑕疵

- `Mode::Text` 映射到 `RenderMode::Compact`(`src/cli/mod.rs:213-214`、`src/types.rs:70-98`)
  ——同一概念两个名字。
- `run_with` 先 `to_config()` 再立刻覆写 `cfg.mode`(`src/cli/mod.rs:223-224`),
  因为 `to_config` 无法在无探测的情况下解析 `Auto`;这个两段式构造值得显式化。
- `main.rs:42` 的 `help_requested` 手工解析 argv 以探测 `--help`,与 clap 自身逻辑重复
  ——对二进制入口可接受,但属 argv/clap 耦合。

---

## 优先级建议

| 编号 | 问题 | 是否**真实**耦合 | 建议 |
|---|---|---|---|
| 1 | 渲染策略在 cli 层 | 是(cli 持有 renderer 内部) | **建议处理** |
| 2 | `Config` 强加给 `input` | 是(领域模块依赖跨域聚合) | **建议处理** |
| 3 | `Symbol`/`Frame` 同构 | 否(结构重复) | 记录,暂不动 |
| 4 | `encode.rs` 混杂分片器 | 否(地址问题) | 第三种符号时再抽 |
| 5 | 命名/构造瑕疵 | 否(可读性) | 顺手修 |

#3–#5 值得知晓但不紧急。**#1 与 #2** 是唯一产生「真实耦合」的两项,
且都是受控的小改动,能让代码对齐 AGENTS.md 自身定下的规则。

---

## 实施记录

### #2 `input` 解耦(已完成)

`input::resolve` 签名由 `(&Config, stdin, is_tty)` 改为
`(&InputSpec, preserve_newline: bool, stdin, is_tty)`(`src/input.rs`);
调用点更新为 `crate::input::resolve(&cfg.input, cfg.preserve_newline, …)`(`src/cli/mod.rs`)。
`input` 模块不再 `use crate::types::Config`,只依赖 `InputSpec`。unit 测试同步改为直接
构造 `InputSpec`。

### #1 渲染分派下沉(已完成)

`render_all`、`KITTY_IMAGE_ID_BASE`、`format_label` 与 ANSI 钉字形逻辑从 `cli/mod.rs`
移入 `render.rs`,并以单一入口暴露:

```rust
pub struct RenderOptions { pub border: u32, pub invert: bool, pub glyphs: GlyphSet }
pub fn render_all(codes: &[Symbol], format: Format, mode: RenderMode,
                  options: RenderOptions, out: &mut dyn Write) -> io::Result<()>
```

`cli::run` 现在只组装 `RenderOptions` 并调用 `render::render_all`;cli 不再引用任何
具体渲染器(`build_ink`/`render_kitty`/`render_sixel`/`render_ansi`/`render_on_field`)、
Kitty image id 或字形钉法。原先落在 `cli/mod.rs` 的 6 个 `render_all_*` 测试随实现一并
迁入 `render.rs`。`lib.rs` 模块文档的流水线同步更新为 `… -> render::render_all -> stdout`。

> 说明:多码标题(caption)最初被标注为「呈现层职责,可留在 cli」;但「每条码一个
> image id」的 Kitty 序号需要与标题在同一循环内交织,故整段多码版式一并下沉到 `render`,
> 由渲染层统一负责。

### 验证

- `cargo fmt --all -- --check` 通过;`cargo clippy --all-targets -- -D warnings` 无告警。
- `cargo test`:172 项全绿(112 unit + 33 CLI + 27 round-trip)。
- 重构前后对 CLI 实际输出做了逐字节指纹比对(text / ansi / quadrant / braille / invert /
  datamatrix / split / stdin-strip 九种场景),**sha256 完全一致**,功能无变化。
