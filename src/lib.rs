//! `qrterm` turns text into a QR code rendered in the terminal.
//!
//! # Pipeline
//!
//! ```text
//! argv / stdin -> input::resolve -> encode::encode -> render::build_ink -> render::render -> stdout
//! ```
//!
//! # Layering
//!
//! Dependencies point downward only:
//! `cli` -> {`input`, `encode`, `render`} -> `types` / `error`.
//! The domain modules are pure and never depend on `clap`; all side effects are
//! confined to [`cli::run`] / [`cli::run_with`].

#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod cli;
pub mod encode;
pub mod error;
pub mod input;
pub mod render;
pub mod types;
