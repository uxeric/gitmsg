//! The one colored mark gitmsg paints, then clears.
//!
//! The chip is void on hot pink, the same hot as the readme hero. The moving
//! signal is ice. A quiet word sits in ghost. Amber is only the two revisions
//! on an update. Sentences stay in the terminal's own foreground. `NO_COLOR`
//! and a dumb terminal get the plain text.

use std::io::{self, IsTerminal};

pub const ICE: &str = "\x1b[38;2;41;240;255m";
pub const AMBER: &str = "\x1b[38;2;246;196;69m";
pub const GHOST: &str = "\x1b[38;2;110;106;130m";
pub const RESET: &str = "\x1b[0m";

/// Void (`#07060f`) on hot (`#ff2a6d`), with a space of padding inside the chip.
pub const CHIP: &str = "\x1b[1;38;2;7;6;15;48;2;255;42;109m gitmsg \x1b[0m";

pub fn stderr_color() -> bool {
    color_allowed() && io::stderr().is_terminal()
}

pub fn stdout_color() -> bool {
    color_allowed() && io::stdout().is_terminal()
}

fn color_allowed() -> bool {
    std::env::var_os("NO_COLOR").is_none()
        && !matches!(std::env::var("TERM").as_deref(), Ok("dumb"))
}

pub fn failure(error: &str, color: bool) -> String {
    if color {
        format!("{CHIP} {error}")
    } else {
        format!("gitmsg: {error}")
    }
}

pub fn version_line(version: &str, rev: &str, color: bool) -> String {
    if color {
        format!("{CHIP} {version} {GHOST}({rev}){RESET}")
    } else {
        format!("gitmsg {version} ({rev})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_chip_leads_a_failure_and_plain_text_keeps_the_prefix() {
        let colored = failure("Could not reach Grok: timed out", true);
        assert!(colored.starts_with(CHIP), "{colored}");
        assert!(colored.contains("Could not reach Grok: timed out"));
        assert!(colored.contains("48;2;255;42;109"), "{colored}");
        assert!(!colored.contains("gitmsg:"), "{colored}");

        assert_eq!(
            failure("Could not reach Grok: timed out", false),
            "gitmsg: Could not reach Grok: timed out"
        );
    }

    #[test]
    fn a_version_line_names_the_build_either_way() {
        let colored = version_line("0.1.0", "abc1234", true);
        assert!(colored.contains("0.1.0"), "{colored}");
        assert!(colored.contains("abc1234"), "{colored}");
        assert!(colored.contains(CHIP), "{colored}");
        assert_eq!(
            version_line("0.1.0", "abc1234", false),
            "gitmsg 0.1.0 (abc1234)"
        );
    }
}
