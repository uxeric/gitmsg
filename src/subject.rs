//! Commit-title cleanup and the `git c -m "..."` line.

pub const SOFT_LIMIT: usize = 50;
pub const HARD_LIMIT: usize = 72;

pub fn char_len(text: &str) -> usize {
    text.chars().count()
}

/// Turn model output into one commit subject, or `None` when nothing usable remains.
pub fn clean_subject(raw: &str) -> Option<String> {
    let mut line = String::new();
    for candidate in raw.lines() {
        let trimmed = candidate.trim();
        if trimmed.is_empty() || trimmed.starts_with("```") {
            continue;
        }
        line = trimmed.to_string();
        break;
    }
    if line.is_empty() {
        return None;
    }

    line = fold_punctuation(&line);
    line = line.trim_matches('`').trim().to_string();
    line = strip_label_prefix(&line);
    line = strip_wrapping_quotes(&line);
    line = strip_list_marker(&line);
    line = strip_wrapping_quotes(&line);

    let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
    let collapsed = strip_trailing_period(&collapsed);
    if collapsed.is_empty() || collapsed.chars().any(|c| c.is_control()) {
        return None;
    }
    let mut chars = collapsed.chars();
    if !chars.next().is_some_and(|c| c.is_ascii_uppercase()) {
        return None;
    }
    Some(collapsed)
}

pub fn trim_to_hard_limit(subject: &str) -> String {
    if char_len(subject) <= HARD_LIMIT {
        return subject.to_string();
    }
    let mut out: String = subject.chars().take(HARD_LIMIT).collect();
    if let Some(index) = out.rfind(' ') {
        if index >= 24 {
            out.truncate(index);
        }
    }
    out.trim_end_matches([' ', '&', ',', ';', ':', '-'])
        .trim()
        .to_string()
}

/// The exact command that should land on the command line. It is not executed.
///
/// `!` is written as `\!` because an interactive zsh history-expands that back
/// to a literal `!` when the line is accepted.
pub fn git_commit_invocation(subject: &str) -> String {
    let mut escaped = String::with_capacity(subject.len() + 16);
    for ch in subject.chars() {
        match ch {
            '\\' | '"' | '$' | '`' | '!' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            _ => escaped.push(ch),
        }
    }
    format!("git c -m \"{escaped}\"")
}

fn fold_punctuation(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\u{2018}' | '\u{2019}' | '\u{201A}' => out.push('\''),
            '\u{201C}' | '\u{201D}' | '\u{201E}' => out.push('"'),
            '\u{2013}' | '\u{2014}' => out.push('-'),
            '\u{2026}' => out.push_str("..."),
            '\u{00A0}' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out
}

fn strip_wrapping_quotes(text: &str) -> String {
    let bytes = text.as_bytes();
    if bytes.len() >= 2 {
        let (first, last) = (bytes[0], bytes[bytes.len() - 1]);
        if (first == b'"' && last == b'"') || (first == b'\'' && last == b'\'') {
            return text[1..text.len() - 1].trim().to_string();
        }
    }
    text.to_string()
}

fn strip_label_prefix(text: &str) -> String {
    const PREFIXES: &[&str] = &[
        "commit message:",
        "commit title:",
        "subject:",
        "title:",
        "commit:",
        "message:",
    ];
    let lower = text.to_ascii_lowercase();
    for prefix in PREFIXES {
        if lower.starts_with(prefix) {
            return text[prefix.len()..].trim().to_string();
        }
    }
    text.to_string()
}

fn strip_list_marker(text: &str) -> String {
    let trimmed = text.trim_start_matches(['-', '*']).trim();
    let mut rest = trimmed;
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        rest = &rest[digits.len()..];
        if let Some(after) = rest.strip_prefix(". ") {
            return after.trim().to_string();
        }
        if let Some(after) = rest.strip_prefix(") ") {
            return after.trim().to_string();
        }
    }
    trimmed.to_string()
}

fn strip_trailing_period(text: &str) -> String {
    if text.ends_with("...") || !text.ends_with('.') {
        return text.to_string();
    }
    text[..text.len() - 1].trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_quotes_period_and_label() {
        let raw = "Commit: \"Updates login & error text.\"";
        assert_eq!(
            clean_subject(raw).as_deref(),
            Some("Updates login & error text")
        );
    }

    #[test]
    fn skips_fences_and_keeps_the_subject() {
        let raw = "```\nAdds the port picker\n```\n";
        assert_eq!(clean_subject(raw).as_deref(), Some("Adds the port picker"));
    }

    #[test]
    fn rejects_a_lowercase_or_empty_reply() {
        assert!(clean_subject("   ").is_none());
        assert!(clean_subject("updates the form").is_none());
        assert!(clean_subject("git c -m \"nope\"").is_none());
    }

    #[test]
    fn folds_smart_punctuation() {
        let raw = "Fixes the crash when a name is “empty”…";
        assert_eq!(
            clean_subject(raw).as_deref(),
            Some("Fixes the crash when a name is \"empty\"...")
        );
    }

    #[test]
    fn trims_on_a_word_boundary() {
        let long = "Updates the scheduling screen so overnight jobs stay visible to managers";
        let trimmed = trim_to_hard_limit(long);
        assert!(char_len(&trimmed) <= HARD_LIMIT);
        assert!(trimmed.starts_with("Updates the scheduling"));
        assert!(!trimmed.ends_with(' '));
    }

    #[test]
    fn quotes_shell_metacharacters_and_keeps_ampersand() {
        let subject = r#"Adds "quotes", $HOME, `ticks` & a bang!"#;
        let command = git_commit_invocation(subject);
        assert!(command.starts_with("git c -m \""));
        assert!(command.contains(r#"\"quotes\""#));
        assert!(command.contains(r#"\$HOME"#));
        assert!(command.contains(r#"\`ticks\`"#));
        assert!(command.contains(r#"\&"#) || command.contains(" & "));
        assert!(command.contains(r#"\!"#));
        assert!(command.contains(" & "));
    }

    #[test]
    fn zsh_receives_the_subject_as_one_argument() {
        let subject = r#"Adds "quotes", $HOME, `ticks` & a note"#;
        let command = git_commit_invocation(subject);
        let output = std::process::Command::new("zsh")
            .args([
                "-c",
                "git() { print -r -- \"$3\"; }\neval \"$1\"\n",
                "zsh",
                &command,
            ])
            .output()
            .expect("zsh");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let got = String::from_utf8(output.stdout).unwrap();
        assert_eq!(got.trim_end(), subject);
    }

    #[test]
    fn bang_is_escaped_for_interactive_history() {
        let command = git_commit_invocation("Fixes the crash!");
        assert!(command.contains(r#"crash\!"#));
    }
}
