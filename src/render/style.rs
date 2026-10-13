//! Explicit SGR colour sequences shared by the character renderers (FR-3.8,
//! FR-3.9).
//!
//! The text and ANSI renderers set explicit colours so a code is a
//! self-contained dark-on-white image — the quiet zone stays white even on a
//! black terminal — rather than one that borrows the terminal's own palette.
//! `invert` swaps the dark and light roles (FR-3.9).

/// Dark modules black on a white field (the default) (FR-3.8).
pub(super) const DARK_ON_LIGHT: &str = "\x1b[30;107m";
/// Light modules on a dark field, for `invert` (FR-3.9).
pub(super) const LIGHT_ON_DARK: &str = "\x1b[97;40m";
/// Reset every SGR attribute.
pub(super) const RESET: &str = "\x1b[0m";
