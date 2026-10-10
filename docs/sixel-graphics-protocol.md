# Sixel 图形协议渲染 · 实现说明

> `qrtxt -s` 用 Sixel 图形协议把二维码画成**像素级黑白位图**，而非 Unicode 字形。
> 状态：已实现（分支 `docs/sixel-support`），测试全绿（139 项）。

**动机**：`-k`（Kitty 图形协议）覆盖 kitty / Ghostty / WezTerm 一小批终端；而 xterm、
Konsole、foot、mlterm、iTerm2、Windows Terminal 等大量终端**不支持 Kitty、但支持 Sixel**。
新增 Sixel 与 Kitty 互补，扩大位图渲染的适用范围。

---

## 一、协议要点（协议层）

每一帧是一条 DCS：`ESC P q <数据> ESC \`。用到的控制功能：

| 功能 | 语法 | 说明 |
|---|---|---|
| DCS 引导 | `ESC P q` | 进入 Sixel 图形模式 |
| 光栅属性 | `" Pan;Pad;Ph;Pv` | 像素宽高比 `1;1`（方形）+ 图像尺寸 `W;H` |
| 颜色引导 | `# Pc;2;Pr;Pg;Pb` | RGB 调色板寄存器，各分量取值 0–100 |
| 数据字符 | `?`–`~`（`0x3F`–`0x7E`） | 每字符一列 6 像素，自下往上 |
| 重复（RLE） | `! N <字符>` | 游程压缩，`N>=4` 才划算 |
| 回车 / 换带 | `$` / `-` | 带内回到最左 / 下移一带 |

- 二维码**纯双色**，只需 2 个调色板寄存器 + RLE，输出比 Kitty 的 base64 RGB 更紧凑。
- 无 image id、无分块：**发送即绘制**，逐码连续输出即可（比 Kitty 更简单）。

---

## 二、技术架构与流程（代码层）

### 数据流

```
input -> encode -> render::render_sixel -> sixel::encode -> stdout
```

`encode` 阶段完全复用；`-s` 只替换最后一步的渲染。

### 模块分工

- **`src/sixel.rs`（纯逻辑）**：协议封帧本身。
  - `encode`：写调色板 + 光栅属性 + 逐带（黑白各一遍）RLE 数据 + 终止。
  - `write_rle`：游程压缩；`da1_query` / `response_has_sixel`：探测序列与判定。
- **`src/render.rs`**：`module_bitmap(qr, border)` 把二维码矩阵放大成方形像素位图
  （`module_px = (300 / side).clamp(2, 8)`，静默区为亮色），`render_kitty` 与
  `render_sixel` 共用；`render_sixel` 再把位图交给 `sixel::encode`。
- **`src/types.rs`**：`RenderMode::Sixel`。
- **`src/cli/`**：`mod.rs` 放 `-s/--sixel` 开关（与 `--kitty`、`--no-compact` 互斥）与渲染
  分支；`probe.rs` 放 DA1 探测；`help.rs` 放帮助提示。

### 编码流程（`sixel::encode`）

1. 写 DCS 引导 `ESC P q`。
2. 定义两个调色板寄存器：`#0`（亮）/ `#1`（暗）；`--invert` 交换二者。
3. 写光栅属性 `"1;1;W;H`，**声明方形像素**，保证模块方正、可扫。
4. 逐带（每 6 行）绘制：先 `#0` 画所有亮像素，`$` 回带首后 `#1` 画所有暗像素，`-` 进下一带。
5. 写终止 `ESC \`。

每列字符 = `0x3F + 6 位掩码`（自下往上对应 6 个像素）；相邻相同字符用 `!N<char>` 压缩，
静默区整带全亮即一条 `!N~`。

### 支持探测（DA1）

Sixel 没有 Kitty 那样的握手，改用**主设备属性（DA1）**：

1. `stdout` 非终端 → 判定“无终端可问”。
2. 打开 `/dev/tty`，用 `rustix` termios 置 **raw 模式**（`VMIN=0`、`VTIME=1`）。
3. 发送 `ESC [ c`，累计读到终止字节 `c` 或 300ms 截止。
4. 应答 `ESC [ ? <参数> c` 中**含 `4`** 即支持。

- **探测语义**：DA1 是尽力而为——支持但不广告 `4` 的终端按“不支持”处理，宁缺毋滥，绝不
  打印画不出的转义字节。探测走 `/dev/tty`，不干扰 stdin 的 payload 流。
- **帮助提示**：`-h` / `--help` 时给 `--sixel` 追加“本终端是否支持”的一行结论；无终端则省略。
- **失败即退出**：用户显式指定 `-s` 后，若 `stdout` 非终端或终端不广告支持，**以用法错误
  退出（退出码 2）**，不打印乱码。

---

## 三、验证

- `cargo fmt --all -- --check`、`cargo clippy --all-targets -- -D warnings` 通过。
- `cargo test`：**139 项通过**（lib 93 + cli 26 + roundtrip 20）。
- 可扫性：`sixel_bitmap_round_trips` / `sixel_multi_code_round_trips` 用测试内置的极简
  Sixel 解码器把输出还原成位图、抽样回模块网格，再交 `rqrr` 解码，覆盖普通 / URL /
  中文 / 反相及多码顺序重组。
- 编码单测：封帧结构、调色板 / 光栅属性、RLE（含 `N<4` 边界）、部分末带、DA1 判定。
- 失败路径：`sixel_requires_a_terminal`（非终端 → 退出码 2）、`--sixel` 与 `--kitty` /
  `--no-compact` 冲突。
- **无新依赖**：解码校验用测试内置的极简解码器完成。

---

## 四、参考

### 终端支持

| 支持 Sixel | 不支持 Sixel |
|---|---|
| xterm、WezTerm、foot、mlterm、contour、Konsole、mintty、iTerm2、Windows Terminal（1.22+） | kitty、Ghostty（主推 Kitty 协议）、Alacritty、macOS Terminal、标准 libvte 系 |

### 注意

- **方形像素（可扫性硬风险）**：传统 Sixel 像素被视为 1:2，若终端不遵守光栅属性会纵向
  拉伸、导致扫不出。实现已声明 `"1;1`，但**需在目标终端目视 + 手机扫码验证**（本开发环境
  无法目视 Sixel）。
- 探测可靠性弱于 Kitty（DA1 非权威握手）。
- tmux / screen 需 DCS 透传配置——与 Kitty 相同，非新增劣势。

### 链接

- Sixel 格式综述：<https://en.wikipedia.org/wiki/Sixel>
- DEC VT3xx SIXEL 图形规范（第 14 章，权威语法）：<https://vt100.net/docs/vt3xx-gp/chapter14.html>
- Sixel 终端支持总览：<https://www.arewesixelyet.com/>
- lsix（兼容终端列表与探测实践）：<https://github.com/hackerb9/lsix>
- DA1 中属性 `4` 表示 SIXEL（alacritty #910）：<https://github.com/alacritty/alacritty/issues/910>
- WezTerm 特性（同时支持 Kitty + Sixel）：<https://wezterm.org/features.html>
- Ghostty 关于 Sixel 的立场（拒绝，主推 Kitty 协议）：<https://github.com/ghostty-org/ghostty/discussions/2496>
- Windows Terminal 1.22 加入 Sixel：<https://devblogs.microsoft.com/commandline/windows-terminal-preview-1-22-release/>
- 同源文档：[`kitty-graphics-protocol.md`](./kitty-graphics-protocol.md)
