//! `--help` enhancement: append a live Kitty-support hint to the `--kitty`
//! option (FR-3.12).
//!
//! The probe runs once, only when help is being shown, so a normal invocation
//! pays nothing. A piped `--help` yields no hint (there is no terminal).

use clap::CommandFactory;

use super::Cli;
use super::probe::probe_kitty_terminal;

/// A one-line hint about the current terminal's Kitty support, for `--help`
/// (FR-3.12).
///
/// Runs the same `a=q` handshake as [`crate::cli::run_with`], but never fails: it
/// returns `None` when there is no terminal to probe or the probe errors, so the
/// help text stays truthful rather than guessing.
#[must_use]
pub fn kitty_support_hint() -> Option<String> {
    hint_for(probe_kitty_terminal().ok().flatten())
}

/// Map a probe result to the help hint; `None` means there was nothing to probe.
fn hint_for(support: Option<bool>) -> Option<String> {
    match support {
        Some(true) => Some("This terminal supports it.".to_string()),
        Some(false) => Some("This terminal does not support it.".to_string()),
        None => None,
    }
}

/// The clap command with a live Kitty support `hint` appended to the `--kitty`
/// option's help.
///
/// Used only when help is being shown, so the probe cost is paid once. The short
/// and long help both gain the hint.
#[must_use]
pub fn command_with_kitty_hint(hint: &str) -> clap::Command {
    Cli::command().mut_arg("kitty", |arg| {
        let short = arg.get_help().map(ToString::to_string).unwrap_or_default();
        let long = arg
            .get_long_help()
            .map(ToString::to_string)
            .unwrap_or_else(|| short.clone());
        // clap strips the trailing stop from the short help, so restore it before
        // appending, otherwise the two sentences run together.
        arg.help(format!("{} {hint}", end_with_period(short.trim_end())))
            .long_help(format!("{}\n\n{hint}", long.trim_end()))
    })
}

/// `text` with a trailing full stop, adding one only when it is missing.
fn end_with_period(text: &str) -> String {
    if text.ends_with('.') {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn support_hint_reflects_the_probe_result() {
        // Tested through the pure mapping: the live probe depends on the ambient
        // terminal, which would make this test fail under an interactive one.
        assert_eq!(hint_for(None), None);
        assert!(hint_for(Some(true)).unwrap().contains("supports it"));
        assert!(hint_for(Some(false)).unwrap().contains("does not support"));
    }

    #[test]
    fn kitty_hint_is_appended_to_both_help_forms() {
        let hint = "This terminal supports the Kitty graphics protocol.";
        let mut command = command_with_kitty_hint(hint);

        let short = command.render_help().to_string();
        assert!(short.contains("--kitty"), "{short}");
        // The stripped stop is restored so the hint starts a new sentence.
        assert!(short.contains(". This terminal supports"), "{short}");

        let long = command.render_long_help().to_string();
        assert!(long.contains("--kitty"), "{long}");
        assert!(long.contains(hint), "{long}");
    }
}
