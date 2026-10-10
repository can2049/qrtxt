# Kitty 图形协议渲染 · 实现说明

> `qrtxt --mode kitty` 用 Kitty 图形协议把二维码画成**像素级黑白位图**，而非 Unicode 字形。
> 状态：已实现并合并（PR #1），测试全绿（117 项）。

**背景**：braille 下暗模块只是一个凸点，三个定位框（finder pattern）被渲染成细点线、
不够醒目。改用位图即可不受字体渲染质量的限制。

---

## 一、协议怎么被支持（协议层）

每一帧是一条 APC：`ESC _ G <控制数据> ; <base64 载荷> ESC \`。本次用到的键：

| 键 | 含义 | 取值 |
|---|---|---|
| `a` | 动作 | `T` = 传输并显示；`q` = 探测支持 |
| `f` | 像素格式 | `24` = 原始 RGB |
| `s` / `v` | 图像宽 / 高（像素） | `side * module_px` |
| `i` | 图像 id | 显示逐码递增；探测 `1000000` |
| `m` | 还有后续分块 | `1` 有、`0` 无 |

- **分块传输**：单块 base64 上限 4096 字符（= 3072 原始字节），除最后一块外都带 `m=1`。
- **原生像素放置**（不指定 `c`/`r`）⇒ 1 图像像素 = 1 屏幕像素 ⇒ 每个模块是方正像素块，
  不受终端单元格宽高比影响。
- **支持探测 `a=q`**：发一条 1×1 探测序列（`...a=q,t=d,f=24;AAAA`），终端回 `;OK` 即支持。
- **图像 id 必须逐码唯一**：`a=T` 遇到重复 id 会替换已存图像并丢弃其放置，只剩最后一码
  可见；故 `render_all` 按 `424242 + index` 逐码分配。
- **光标**：`a=T` 默认把光标移到图像之后；实现依赖该默认行为，再补一个 `\n`。

---

## 二、技术实现（代码层）

### 数据流

```
input -> encode -> render::render_kitty -> kitty::transmit_rgb -> stdout
```

`encode` 阶段完全复用；`--mode kitty` 只替换最后一步的渲染。

### 模块分工

- **`src/kitty.rs`（纯逻辑）**：协议封帧本身。
  - `encode_base64`：手写 base64（不引额外依赖）。
  - `transmit_rgb`：按 3072 字节分块，首块带全部控制键，末块 `m=0`。
  - `query`：探测序列；`response_ok`：判定响应是否含 `;OK`。
- **`src/render.rs`**：`render_kitty(qr, border, invert, id, out)` 把 QR 矩阵转 RGB 位图：
  `side = qr.width() + 2*border`，`module_px = (300/side).clamp(2,8)`；
  暗模块→黑、亮模块→白（`--invert` 交换），静默区为亮色。
- **`src/types.rs`**：新增 `RenderMode::Kitty`。
- **`src/cli/`**：`mod.rs` 放 `-M/--mode` 开关（值 `auto`/`text`/`ansi`/`kitty`/`sixel`）与逐码 id 分配；
  `probe.rs` 放 Kitty/Sixel 合并探测；`help.rs` 放帮助提示。

### 支持探测（`a=q` 握手，与 DA1 合并为一次往返）

读**控制终端**而非 stdin，避免与被管道输入的 payload 争抢：

1. `stdout` 非终端 → 直接判定为“无终端可问”。
2. 打开 `/dev/tty` 读写。
3. 用 `rustix` 的 termios 置 **raw 模式**，并令 `VMIN=0`、`VTIME=1`（0.1s 读超时）。
4. 一次写入 Kitty 的 `a=q` 探测序列与 Sixel 的 DA1 查询，累计读取直到两种应答都到齐
   （或 300ms 截止）；`auto` 只需二者之一，故命中 Kitty 后即提前结束。
5. `TermiosGuard`（`Drop`）保证任何返回路径都恢复原始终端属性。

含 `;OK` → 支持 Kitty；DA1 含 `4` → 支持 Sixel；超时 / 无响应 → 该协议不支持。

> 合并探测的原因：两个探测读同一个 `/dev/tty` 输入流，并发读取会争抢字节、`tcsetattr`
> 也会互相竞争，因此不用多线程，而是把两次查询并成一次往返，`auto` 最坏只等一个 300ms 窗口。

### 帮助里的支持提示

`-h` / `--help` 时 `main` 先探测一次，把“`auto` 在本终端会选哪一种”拼进 `--mode` 的
短/长帮助（`cli::mode_support_hint` + `cli::add_mode_hint`）。仅当 `stdout` 是终端才探测；
探测失败或没有终端时静默省略（不猜测）。

### 失败即退出

按需求，用户显式指定 `--mode kitty`（或 `--mode sixel`）后，若 `stdout` 非终端或终端不支持，
**以用法错误退出（退出码 2）**，不打印终端无法理解的乱码。新增依赖：
`rustix = { version = "1", features = ["termios"] }`（crate 仍 `#![forbid(unsafe_code)]`）。

---

## 三、验证

- `cargo test`：**142 项通过**（lib 96 + cli 26 + roundtrip 20），无警告。
- 可扫性：`kitty_bitmap_round_trips` / `kitty_multi_code_round_trips` 把 APC 解析回模块网格
  再交 `rqrr` 解码，覆盖普通 / URL / 中文 / 反相及多码顺序重组。
- id 唯一：`render_all_kitty_renders_every_code_with_its_own_image_id`（改回共用 id 立刻失败）。
- 帮助提示：`mode_hint_is_appended_to_both_help_forms` 与
  `hint_reflects_the_probe_result`（纯映射，不依赖真实终端）。
- 探测路径：`tests/cli.rs::kitty_mode_requires_a_terminal` 覆盖“stdout 非终端 → 退出码 2”；
  真实终端的 `;OK` / 超时分支**无自动化测试**（需 pty），仅在实机手测（见第四节）。
- 错误路径已实测：`stdout` 非终端（`--mode kitty`），退出码 2。

---

## 四、下一步与注意

- **实机确认（阻塞项）**：在 kitty / Ghostty / WezTerm 下核对显示效果与 `a=T` 之后的
  光标 / 换行排版（本开发环境为 VS Code 终端，**无法目视验证**；可扫性已由测试保证）。
- 可选增强：每模块像素倍率可配置、显示后清理图像。Sixel 已作为并列模式实现
  （`--mode sixel`），见 [sixel-graphics-protocol.md](./sixel-graphics-protocol.md)。

参考：Kitty 图形协议规范 https://sw.kovidgoyal.net/kitty/graphics-protocol/ ；
`rustix` termios https://docs.rs/rustix/latest/rustix/termios/ 。
