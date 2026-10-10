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
  `Cli` schema and orchestration, `probe.rs` the Kitty capability probe, `help.rs` the
  `--help` hint. Put new pure logic in a domain module, not in `cli`.
- Non-ASCII payloads are prefixed with a UTF-8 ECI, and Kanji-mode segments are force-
  downgraded to bytes (`src/encode.rs`). This stops scanners from decoding UTF-8 CJK as
  Shift-JIS. Do not remove it; it is covered by dedicated tests.
- The crate is `#![forbid(unsafe_code)]` and `#![warn(missing_docs)]`: every public item
  and every module needs a doc comment (each file starts with a `//!` header).
- Comments cite requirement IDs (`FR-x.y`, `AC-x`, `ADR-x`) from an external spec that is
  NOT in this repo. Preserve existing citations and add them for new behavior.

## Error and exit-code contract

- `AppError` maps to exit codes: `Usage` / `Input` -> `2`, `Io` -> `1` (`src/error.rs`).
- Error messages must NEVER include the payload — it may hold secrets such as tokens
  (FR-6.2). This is deliberate; keep it when adding error paths.
- A closed downstream pipe (`BrokenPipe`, e.g. `qrtxt ... | head`) is treated as success.

## Testing

- Run from the crate root: `cargo test`. It covers unit tests (in each module), behavior
  (`tests/cli.rs`), and scannability (`tests/roundtrip.rs`).
- `tests/roundtrip.rs` renders the CLI output, parses it back into the module grid, and
  decodes it with `rqrr` to prove the printed code still scans. Add round-trip coverage
  for any new glyph, rendering, or encoding behavior.
- `tests/roundtrip.rs` reads `README.zh-CN.md` from the working directory; keep that file present.
- CI (`.github/workflows/rust.yml`) runs `cargo build`, `cargo clippy --all-targets -- -D
  warnings`, and `cargo test`. There is no rustfmt gate, and no formatter config in the repo.

## Docs and commits

- Keep `README.md` and `README.zh-CN.md` in sync when changing user-facing options.
- Commit messages are imperative and capitalized, with no Conventional-Commits prefix
  (e.g. "Declare UTF-8 and avoid Kanji segments for non-ASCII text").
