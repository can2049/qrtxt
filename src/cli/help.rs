//! `--help` enhancement: append a live support hint to the `--mode` option
//! (FR-3.10, FR-3.12, FR-3.13).
//!
//! The probe runs once, only when help is being shown, so a normal invocation
//! pays nothing. A piped `--help` yields no hint (there is no terminal).

use clap::Command;

use super::probe::{Capabilities, probe_capabilities};

/// A one-line hint about what `--mode auto` will choose on the current terminal,
/// for `--help` (FR-3.10).
///
/// Runs the same fused probe as [`crate::cli::run_with`], but never fails: it
/// returns `None` when there is no terminal to probe or the probe errors, so the
/// help text stays truthful rather than guessing.
#[must_use]
pub fn mode_support_hint() -> Option<String> {
    describe(probe_capabilities().ok().flatten().as_ref())
}

/// The hint sentence for a probe result; `None` means there was nothing to probe.
fn describe(support: Option<&Capabilities>) -> Option<String> {
    let capabilities = support?;
    Some(if capabilities.kitty {
        "auto: this terminal supports the Kitty graphics protocol.".to_string()
    } else if capabilities.sixel {
        "auto: this terminal supports the Sixel graphics protocol.".to_string()
    } else {
        "auto: this terminal has no graphics support, so text is used.".to_string()
    })
}

/// `command` with the live support `hint` appended to the `--mode` option's help
/// (FR-3.10).
#[must_use]
pub fn add_mode_hint(command: Command, hint: &str) -> Command {
    with_hint(command, "mode", hint)
}

/// Append `hint` to the short and long help of the `option` named `name`.
///
/// Used only when help is being shown, so the probe cost is paid once. The short
/// and long help both gain the hint.
fn with_hint(command: Command, name: &str, hint: &str) -> Command {
    command.mut_arg(name, |arg| {
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
    use crate::cli::Cli;
    use clap::CommandFactory;

    fn capabilities(kitty: bool, sixel: bool) -> Capabilities {
        Capabilities { kitty, sixel }
    }

    #[test]
    fn hint_reflects_the_probe_result() {
        // Tested through the pure mapping: the live probe depends on the ambient
        // terminal, which would make this test fail under an interactive one.
        assert_eq!(describe(None), None);
        assert!(
            describe(Some(&capabilities(true, true)))
                .unwrap()
                .contains("Kitty")
        );
        // Kitty is preferred, so a Sixel-only terminal is described by Sixel.
        assert!(
            describe(Some(&capabilities(false, true)))
                .unwrap()
                .contains("Sixel")
        );
        assert!(
            describe(Some(&capabilities(false, false)))
                .unwrap()
                .contains("text")
        );
    }

    #[test]
    fn mode_hint_is_appended_to_both_help_forms() {
        let hint = "auto: this terminal supports the Kitty graphics protocol.";
        let mut command = add_mode_hint(Cli::command(), hint);

        let short = command.render_help().to_string();
        assert!(short.contains("--mode"), "{short}");
        // The stripped stop is restored so the hint starts a new sentence.
        assert!(short.contains(". auto: this terminal"), "{short}");

        let long = command.render_long_help().to_string();
        assert!(long.contains("--mode"), "{long}");
        assert!(long.contains(hint), "{long}");
    }
}
