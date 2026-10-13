//! `qrtxt` turns text into a QR code rendered in the terminal.
//!
//! # Pipeline
//!
//! ```text
//! argv / stdin -> input::resolve -> encode::encode_multi -> render::render_all -> stdout
//! ```
//!
//! # Layering
//!
//! Dependencies point downward only:
//! `cli` -> {`input`, `encode`, `render`} -> `types` / `error`, with the
//! low-level `kitty` and `sixel` protocol helpers shared by `cli` (the terminal
//! probe) and `render` (bitmap transmission). `render` is itself split by
//! strategy — `frame` builds the ink grid, `text` / `ansi` draw it with
//! characters, `bitmap` scales it for the graphics protocols — behind the
//! `render::render_all` dispatch.
//! The domain modules never depend on `clap`: `encode` / `render` / `kitty` /
//! `sixel` are pure, and `input` only reads the payload source. All argument
//! parsing and all process-level side effects (stdin/stdout locking, the TTY
//! probe, writing output) live in [`cli`]: [`cli::parse`] turns argv into the
//! schema, and [`cli::run`] / [`cli::run_with`] perform the run.
//!
//! Encoding hands rendering a symbology-agnostic [`types::Symbol`] (a dark/light
//! module grid) rather than a concrete barcode type, so only `encode` depends on
//! a barcode library and a new symbology is added there without touching
//! rendering.

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cli;
pub mod encode;
pub mod error;
pub mod input;
pub mod kitty;
pub mod render;
pub mod sixel;
pub mod types;
