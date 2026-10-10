//! `qrtxt` turns text into a QR code rendered in the terminal.
//!
//! # Pipeline
//!
//! ```text
//! argv / stdin -> input::resolve -> encode::encode_multi -> render::build_ink -> render::render -> stdout
//! ```
//!
//! # Layering
//!
//! Dependencies point downward only:
//! `cli` -> {`input`, `encode`, `render`} -> `types` / `error`, with the
//! low-level `kitty` and `sixel` protocol helpers shared by `cli` (the terminal
//! probe) and `render` (bitmap transmission).
//! The domain modules never depend on `clap`: `encode` / `render` / `kitty` /
//! `sixel` are pure, and `input` only reads the payload source. All argument
//! parsing and all process-level side effects (stdin/stdout locking, the TTY
//! probe, writing output) live in [`cli::run`] / [`cli::run_with`].

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
