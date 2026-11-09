// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_export::Language;
use p4kx_extraction::LocalizationSet;

/// Expected display texts parsed directly from decoded source lines.
pub(crate) struct Oracle(HashMap<String, String>);

#[derive(Default)]
struct Lines<'a> {
    plain: Option<&'a str>,
    suffixed: Option<&'a str>,
}

impl Oracle {
    pub(crate) fn of(set: &LocalizationSet, language: Language) -> Result<Self, String> {
        let source = set
            .entry(language)
            .ok_or_else(|| format!("the Archive holds {language}"))?;
        Ok(Self::parse(source.text().text()))
    }

    pub(crate) fn parse(source: &str) -> Self {
        let mut lines: HashMap<String, Lines<'_>> = HashMap::new();
        for line in source.trim_start_matches('\u{feff}').lines() {
            let Some((key, text)) = line.split_once('=') else {
                continue;
            };
            let (key, suffixed) = key
                .split_once(',')
                .map_or((key, false), |(key, _)| (key, true));
            let key = key.trim().to_ascii_lowercase();
            if key.is_empty() {
                continue;
            }
            let entry = lines.entry(key).or_default();
            if suffixed {
                entry.suffixed = Some(text);
            } else {
                entry.plain = Some(text);
            }
        }
        Self(
            lines
                .into_iter()
                .filter_map(|(key, lines)| {
                    let text = lines
                        .plain
                        .filter(|text| !text.is_empty())
                        .or(lines.suffixed)
                        .or(lines.plain)?;
                    let shown = text.replace("\\n", "\n").trim().to_owned();
                    Some((key, shown))
                })
                .collect(),
        )
    }

    pub(crate) fn text(&self, key: &str) -> Option<&str> {
        let key = key.strip_prefix('@').unwrap_or(key).to_ascii_lowercase();
        self.0
            .get(&key)
            .map(String::as_str)
            .filter(|text| !text.is_empty())
    }
    pub(crate) fn canonical_text(&self, key: &str) -> Option<&str> {
        let key = key.strip_prefix('@').unwrap_or(key).to_ascii_lowercase();
        self.0.get(&key).map(String::as_str)
    }
}
