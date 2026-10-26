# AGENTS.md

This file provides guidance to the AI agent when working with code in this repository.

## What this is

`qrtxt` — a Rust CLI (edition 2024, MSRV 1.85) that renders a QR code as Unicode
terminal text. It is a library (`src/lib.rs`) plus a thin binary (`src/main.rs`).

## Architecture invariants (do not break)

- Dependencies point one way: `cli` -> {`input`, `encode`, `render`} -> `types` / `error`.
- The domain modules must NEVER depend on `clap`. `encode`, `render`, and `kitty` are
  pure (they do no IO); `input` only reads the payload source (a file, or the injected
  stdin reader). All argument parsing and all process-level side effects (locking
  stdin/stdout, probing the TTY, writing output) live in `src/cli/`: `mod.rs` holds the
  `Cli` schema, the `Cli -> Config` mapping and orchestration, `parse.rs` the argv entry
  point, `probe.rs` the fused Kitty+Sixel capability probe, `help.rs` the `--help` hint,
  `value.rs` the clap `ValueEnum` impls for the `types` value enums (they live here, not in
  `types.rs`, to keep the domain clap-free).
  Put new pure logic in a domain module, not in `cli`. `render` is a directory of
  single-responsibility modules (`frame` builds the ink grid, `text`/`ansi` draw it,
  `bitmap` scales it for the graphics protocols, `style` holds the SGR colours) unified
  by the `render::render_all` dispatch; add a module plus a `RenderMode` arm for a new
  renderer instead of growing one function.
- Rendering is chosen by a single `--mode` selector
  (`auto`/`text`/`ansi`/`bitmap`/`kitty`/`sixel`).
  `auto`, the default, is resolved by `cli::resolve_mode` (called from `cli::run_with`)
  against the terminal's capabilities:
  it picks a bitmap only when `stdout` is a terminal and a protocol is supported (Kitty
  before Sixel), so a piped or redirected run always yields text. `bitmap` prefers Kitty
  then Sixel the same way but errors instead of falling back to text. Both protocols are
  asked in ONE round trip (`probe.rs`) — they share one terminal input stream (`/dev/tty`
  on Unix, the `CONIN$` console queue on Windows through the `qrtxt-winprobe` workspace
  member), so never probe them from separate threads.
- The character renderers (`text`/`ansi`) set explicit colours, so the code is a
  self-contained dark-on-white image with a white quiet zone whatever the terminal's own
  colours are; a black-background terminal would otherwise merge the quiet zone with the
  dark modules. `render_on_field` wraps the block glyphs and resets the colour before every
  newline, so the background cannot stretch across the rest of the row. `--invert` swaps the
  two colours. Do not revert to drawing the light modules in the terminal foreground.
- Non-ASCII payloads are prefixed with a UTF-8 ECI, and Kanji-mode segments are force-
  downgraded to bytes (`src/encode.rs`). This stops scanners from decoding UTF-8 CJK as
  Shift-JIS. Do not remove it; it is covered by dedicated tests.
- The crate is `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]`: every public item
  and every module needs a doc comment (each file starts with a `//!` header). The
  `qrtxt-winprobe` workspace member is the only place with unsafe: its private
  `src/win32.rs` opts in function by function, and the rest of that crate plus all of
  `qrtxt` stay safe.
- Comments cite requirement IDs (`FR-x.y`, `AC-x`, `ADR-x`) from an external spec that is
  NOT in this repo. Preserve existing citations and add them for new behavior.

## Error and exit-code contract

- `AppError` maps to exit codes: `Usage` / `Input` -> `2`, `Io` -> `1` (`src/error.rs`).
- Error messages must NEVER include the payload — it may hold secrets such as tokens
  (FR-6.2). This is deliberate; keep it when adding error paths.
- A closed downstream pipe (`BrokenPipe`, e.g. `qrtxt ... | head`) is treated as success.

## Testing

- Run from the crate root: `cargo test`. It runs the default workspace members: the
  `qrtxt` suites plus the `qrtxt-winprobe` unit tests, whose polling/decode logic is
  platform-independent and therefore also covered on Linux CI.
- `tests/roundtrip.rs` renders the CLI output, parses it back into the module grid, and
  decodes it with `rqrr` to prove the printed code still scans. Add round-trip coverage
  for any new glyph, rendering, or encoding behavior.
- `tests/roundtrip.rs` reads `README.zh-CN.md` from the working directory; keep that file present.
- CI (`.github/workflows/rust.yml`) checks formatting first on Linux
  (`cargo fmt --all -- --check`, to fail fast), then runs `cargo build`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test` on Linux and Windows.
  There is no rustfmt config file; the default style applies.

## Docs and commits

- Keep `README.md` and `README.zh-CN.md` in sync when changing user-facing options.
- Commit messages are imperative and capitalized, with no Conventional-Commits prefix
  (e.g. "Declare UTF-8 and avoid Kanji segments for non-ASCII text").

## Publishing

- `qrtxt` depends on the `qrtxt-winprobe` workspace member by version and path. Publish
  that member first (`cargo publish -p qrtxt-winprobe`), then `qrtxt`: the root package
  excludes the member's sources, so the published qrtxt resolves the helper from
  crates.io.
- Pushing a `v*` tag runs `.github/workflows/release.yml`. It refuses a tag whose version
  differs from `Cargo.toml` or that is not yet merged into `main`, builds the target
  matrix there, attaches the archives plus `SHA256SUMS` to the GitHub release, and emits
  build provenance attestations. The release job is idempotent.
- Exercise the pipeline without publishing by running
  `gh workflow run release.yml -f tag=vX.Y.Z -f dry_run=true` (the workflow file must
  already be on the default branch).
