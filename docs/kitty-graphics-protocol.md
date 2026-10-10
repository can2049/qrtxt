# Kitty 图形协议渲染 · 任务总结

> 状态：已实现，测试全绿（113 项），**尚未提交**。
> 日期：2026-10-09 　模块：`qrtxt`　相关命令：`qrtxt -k/--kitty`

本文档记录本次为 `qrtxt` 增加 **Kitty graphics protocol** 位图渲染的任务目标、技术路线
与下一步计划。用户手册见 `README.md` / `README.zh-CN.md`，此处是任务档案。

---

## 一、任务目标

**起因**：braille 字形集虽然密度最高，但一个暗模块只对应一个凸点，三个定位框
（finder pattern）渲染成细点线，不够醒目。

**目标**：新增一种渲染方式，用 Kitty 图形协议把二维码画成**像素级清晰的黑白位图**，
不再受字体块字符渲染质量的限制。

**约束**：

1. 新增能力必须在用户显式指定时才启用，**不做默认**。
2. 用户指定后，若终端不支持该协议，必须**报错并退出**，而不是打印终端无法理解的乱码。
3. 复用既有管线（`input -> encode -> render`），不另起一套编码流程。

---

## 二、技术路线

### 2.1 协议要点（对照 Kitty 官方规范）

- **封帧**：`ESC _ G <控制数据> ; <base64 载荷> ESC \`（APC 转义序列）。
- **载荷分块**：单块 base64 上限 4096 字符 = **3072 原始字节**；除最后一块外都带 `m=1`。
- **传输并显示**：`a=T`，原始 RGB `f=24`，`s=<宽>`、`v=<高>`、`i=<图像 id>`。
- **原生像素放置**（不指定 `c`/`r`）⇒ 1 图像像素 = 1 屏幕像素 ⇒ 模块为方正像素块。
- **支持探测**：发送 `ESC_Gi=<id>,a=q,t=d,f=24;AAAA ESC\`（1×1 像素），
  终端回 `ESC_Gi=<id>;OK ESC\` 表示支持，否则报错。
- **光标**：`a=T` 默认把光标移动到图像之后；实现依赖该默认行为并追加一个 `\n`。

### 2.2 支持探测（`a=q` 握手）

在 `cli` 中实现，读取**控制终端**而非 stdin，避免与被管道输入的 payload 争抢：

- 先要求 `stdout` 是终端（否则无法显示，直接用法错误）。
- 打开 `/dev/tty` 读写。
- 用 `rustix` 的 termios 将 tty 设为 **raw 模式**，并令 `VMIN=0`、`VTIME=1`
  （0.1s 读超时），发送探测序列，累计读到 `ESC\` 或 300ms 截止。
- 通过 `TermiosGuard`（`Drop`）确保任何返回路径都恢复原始终端属性。
- 结果：`;OK` 视为支持；超时/无响应/非 `OK` 视为不支持。

### 2.3 渲染与分层

依赖单向：`cli` → {`input`,`encode`,`render`} → `types`/`error`，另有底层 `kitty`
被 `cli`（探测）与 `render`（传输）共用。

- **`kitty`（新增，纯逻辑）**：协议封帧、base64、分块、探测序列、响应解析。
- **`render::render_kitty`**：把 QR 矩阵转成 RGB 位图 ——
  `side = qr.width() + 2*border` 模块；`module_px = (300/side).clamp(2,8)`；
  暗模块→黑、亮模块→白（`--invert` 交换），静默区为亮色；随后交给
  `kitty::transmit_rgb` 并补一个换行。
- **`cli`**：负责 `-k/--kitty` 开关、模式选择、探测、错误映射；`run` 仍只写字节，
  便于在无终端环境下测试。

### 2.4 CLI 与退出码

- 新增 `-k, --kitty`（与 `--no-compact` 互斥）；`--glyphs` 在位图模式下被忽略。
- 退出码沿用既有约定：`stdout` 非终端 / 协议不支持 → 用法错误 **退出码 2**。

### 2.5 帮助里的支持提示

`qrtxt --help` / `-h` 时，`main` 先请求一次探测，把「当前终端是否支持」这句话拼到
`--kitty` 选项说明的短/长帮助末尾（`cli::kitty_support_hint` + `cli::command_with_kitty_hint`）。
仅当 `stdout` 是终端才探测，管道里的 `-h` 不加提示；探测失败一律静默、只省略提示。

---

## 三、已完成改动

| 文件 | 改动 |
|---|---|
| `Cargo.toml` | 新增依赖 `rustix = { version = "1", features = ["termios"] }`（安全 termios；crate 仍 `#![forbid(unsafe_code)]`） |
| `src/kitty.rs` | **新增**：`encode_base64`、`transmit_rgb`（分块 `a=T,f=24`）、`query`、`response_ok` 及单元测试 |
| `src/render.rs` | 新增 `render_kitty`（QR → RGB 位图）及单元测试 |
| `src/types.rs` | 新增 `RenderMode::Kitty` |
| `src/cli.rs` | `-k/--kitty` 开关；`ensure_kitty_supported` / `probe_kitty` / `TermiosGuard`；`render_all` 增加 Kitty 分支；`run_with` 进入前探测 |
| `src/lib.rs` | 注册 `pub mod kitty;`，更新分层说明 |
| `README.md` / `README.zh-CN.md` | 新增特性、选项、示例、“Kitty 图形协议”小节与实现说明（双语同步） |
| `tests/roundtrip.rs` | 新增 `kitty_bitmap_round_trips`（位图可扫验证） |
| `tests/cli.rs` | 新增终端要求、参数冲突、帮助可见性测试 |

关键参数：显示图像 id 从 `424242` 起**逐码递增**（每个二维码一个 id），探测 id
`1000000`；探测超时 300ms。

> **修复（2026-10-09）**：多码输出原本给每个二维码复用同一个图像 id `424242`。
> `a=T` 用重复 id 传输时，终端会替换已存图像并丢弃其放置（placement），结果只剩
> 最后一个码可见，前面的码全部消失（表现为 `qrtxt -f README.zh-CN.md -k` 只显示
> 第 3 个二维码）。现改为：`render_kitty` 接收 id 参数，`cli::render_all` 按
> `424242 + index` 逐码分配；回归测试 `render_all_kitty_renders_every_code_with_its_own_image_id`
> 断言每个码各发一次图像、id 有序且互不相同。

---

## 四、测试与验证

- `cargo test`：**117 项全部通过**（lib 76 + cli 23 + roundtrip 18），无警告。
- **可扫性**：`kitty_bitmap_round_trips` 把渲染出的 APC 序列解析回模块网格，
  再交给 `rqrr` 解码，覆盖普通 / URL / 中文 / 反相 payload。
- **多码回归**：`render_all_kitty_renders_every_code_with_its_own_image_id`（逐码 id 唯一）
  与 `kitty_multi_code_round_trips`（多码各自渲染成可解码位图并顺序重组）；把
  `render_all` 改回复用同一 id 会让前者立刻失败。
- **帮助提示**：`kitty_hint_is_appended_to_both_help_forms` 断言提示拼进短/长帮助；
  另用 pty 模拟终端验证——回复 `;OK` 显示 `This terminal supports it.`、无回复（超时）
  显示 `This terminal does not support it.`，管道里的 `-h` 不带提示。
- **错误路径已实测**：
  - `qrtxt --kitty hello`（stdout 非终端）→ `kitty: standard output is not a terminal`，退出码 2。
  - `qrtxt --kitty --no-compact hello` → clap 冲突报错，退出码 2。
  - 非支持终端 → 握手超时 → `... does not support the Kitty graphics protocol ...`，退出码 2。

---

## 五、下一步计划

1. **实机确认**（阻塞项）：在 kitty / Ghostty / WezTerm 下人工确认显示效果，重点看
   `a=T` 之后的**光标与换行排版**是否符合预期。
2. **可配置化**（可选）：把每模块像素倍率/目标尺寸（当前 `300px`、`clamp(2,8)`）
   做成参数，便于按屏幕缩放。
3. **图像生命周期**（可选）：已为多码输出逐码分配唯一 id；仍可增加显示后清理/复用，
   避免长列表在高频运行时在终端残留图像。
4. **渲染备选**（可选）：评估 Unicode 占位符模式（随文本流排版）与 Sixel 回退，
   扩大终端兼容面。
5. **提交**：当前改动尚未提交；确认后再按仓库约定（祈使句、首字母大写、无前缀）
   提交。

---

## 六、风险与未验证项

- `a=T` 后光标位置因终端而异；实现用“默认移动 + 追加 `\n`”，**需实机复核**。
- 本开发环境为 VS Code 终端，**无法目视验证位图外观**；可扫性已由测试保证。
- `/dev/tty` 为 Unix 专有，非 Unix 平台不适用。
- 大符号的 base64 载荷由 `module_px` 钳制以控制体积。

---

## 七、参考

- Kitty graphics protocol（官方规范）：https://sw.kovidgoyal.net/kitty/graphics-protocol/
- `rustix` termios API：https://docs.rs/rustix/latest/rustix/termios/
- 仓库内相关文档：`README.md`、`README.zh-CN.md`
