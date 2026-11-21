// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Renders the Rust-crate part of `THIRD-PARTY-NOTICES.md`: a summary table of every crate, then
//! one section per licence text with the crates that use it.

use std::collections::BTreeMap;
use std::fmt::Write as _;

/// A crate that uses a licence text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Crate {
    pub(super) name: String,
    pub(super) version: String,
    pub(super) repository: Option<String>,
    pub(super) licence_source: Option<String>,
}

impl Crate {
    /// The verified licence source, repository or crates.io page.
    fn link(&self) -> String {
        self.licence_source
            .as_ref()
            .or(self.repository.as_ref())
            .cloned()
            .unwrap_or_else(|| format!("https://crates.io/crates/{}/{}", self.name, self.version))
    }
}

/// One licence text and the crates that use it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Licence {
    pub(super) name: String,
    pub(super) text: String,
    pub(super) used_by: Vec<Crate>,
}

/// Renders the summary table and the licence sections, deterministically and ending in a newline.
pub(super) fn render(licences: &[Licence]) -> String {
    let mut out = String::from("### Crates\n\n");
    out.push_str("| Crate | Version | Licence | Link |\n| --- | --- | --- | --- |\n");
    for ((name, version), (link, names)) in table_rows(licences) {
        let licence = names
            .iter()
            .map(|licence_name| escape_cell(licence_name))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "| `{name}` | {version} | {licence} | <{link}> |");
    }
    out.push_str("\n### Licence texts\n");
    for licence in licences {
        section(&mut out, licence);
    }
    out
}

/// One row per crate version, with the names of its licences, in crate order.
fn table_rows(licences: &[Licence]) -> BTreeMap<(&str, &str), (String, Vec<&str>)> {
    let mut rows: BTreeMap<(&str, &str), (String, Vec<&str>)> = BTreeMap::new();
    for licence in licences {
        for krate in &licence.used_by {
            let (_, names) = rows
                .entry((krate.name.as_str(), krate.version.as_str()))
                .or_insert_with(|| (krate.link(), Vec::new()));
            if !names.contains(&licence.name.as_str()) {
                names.push(licence.name.as_str());
            }
        }
    }
    rows
}

fn section(out: &mut String, licence: &Licence) {
    let _ = write!(out, "\n#### {}\n\nUsed by:\n\n", licence.name);
    for krate in &licence.used_by {
        let _ = writeln!(
            out,
            "- `{}` {} (<{}>)",
            krate.name,
            krate.version,
            krate.link()
        );
    }
    let fence = fence_for(&licence.text);
    let text = licence.text.trim_end();
    let _ = write!(out, "\n{fence}text\n{text}\n{fence}\n");
}

/// A backtick fence longer than any backtick run that starts a line of `text`, so the text can
/// never close the block early.
fn fence_for(text: &str) -> String {
    let longest_run = text
        .lines()
        .map(|line| line.trim_start().chars().take_while(|&c| c == '`').count())
        .max()
        .unwrap_or(0);
    "`".repeat(longest_run.max(2) + 1)
}

fn escape_cell(text: &str) -> String {
    text.replace('|', "\\|")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn krate(name: &str, version: &str, repository: Option<&str>) -> Crate {
        Crate {
            name: name.to_owned(),
            version: version.to_owned(),
            repository: repository.map(str::to_owned),
            licence_source: None,
        }
    }

    fn licence(name: &str, text: &str, used_by: Vec<Crate>) -> Licence {
        Licence {
            name: name.to_owned(),
            text: text.to_owned(),
            used_by,
        }
    }

    #[test]
    fn table_lists_each_crate_with_version_licence_and_link() {
        let out = render(&[
            licence(
                "MIT License",
                "text",
                vec![
                    krate("zeta", "2.0.0", Some("https://example.org/zeta")),
                    krate("alpha", "1.0.0", None),
                ],
            ),
            licence(
                "Apache License 2.0",
                "text",
                vec![krate("alpha", "1.0.0", None)],
            ),
        ]);
        let expected = "| Crate | Version | Licence | Link |\n| --- | --- | --- | --- |\n\
             | `alpha` | 1.0.0 | MIT License, Apache License 2.0 | <https://crates.io/crates/alpha/1.0.0> |\n\
             | `zeta` | 2.0.0 | MIT License | <https://example.org/zeta> |\n";
        assert!(
            out.contains(expected),
            "rows are sorted by crate and merge the licences of a crate: {out}"
        );
    }

    #[test]
    fn pipe_in_a_licence_name_does_not_split_the_cell() {
        let out = render(&[licence("A | B", "text", vec![krate("c", "1.0.0", None)])]);
        assert!(
            out.contains("| A \\| B |"),
            "the pipe is escaped in the table: {out}"
        );
    }

    #[test]
    fn each_licence_text_gets_a_section_with_its_users() {
        let out = render(&[licence(
            "ISC License",
            "Permission to use.\n\n",
            vec![krate("ring", "0.17.14", Some("https://example.org/ring"))],
        )]);
        let expected = "#### ISC License\n\nUsed by:\n\n- `ring` 0.17.14 (<https://example.org/ring>)\n\n```text\nPermission to use.\n```\n";
        assert!(
            out.contains(expected),
            "section with users and a fenced text: {out}"
        );
    }

    #[test]
    fn text_with_backticks_gets_a_longer_fence() {
        let text = "``AS IS''\n```\ninside\n````rust\n";
        assert_eq!(fence_for(text), "`````", "longer than the longest run");
        let out = render(&[licence("L", text, vec![krate("c", "1.0.0", None)])]);
        assert!(
            out.contains("`````text\n``AS IS''\n```\ninside\n````rust\n`````\n"),
            "the text sits whole inside the longer fence: {out}"
        );
    }

    #[test]
    fn plain_text_uses_the_shortest_fence() {
        assert_eq!(fence_for("no fences here, only `inline` code"), "```");
        assert_eq!(
            fence_for("``AS IS''"),
            "```",
            "two backticks stay below three"
        );
    }

    #[test]
    fn indented_backtick_run_counts_too() {
        assert_eq!(fence_for("   ````\n"), "`````");
    }

    #[test]
    fn output_is_deterministic() {
        let input = [licence("L", "t", vec![krate("c", "1.0.0", None)])];
        assert_eq!(render(&input), render(&input));
    }
}
