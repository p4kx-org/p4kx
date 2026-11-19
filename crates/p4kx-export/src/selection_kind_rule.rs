// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// How the Records of one kind are selected: by record-path prefix and by type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct KindRule {
    /// The starts of the record path of which one must match, ignoring ASCII case and the kind
    /// of separator.
    pub prefixes: &'static [&'static str],
    /// The name of the Records' struct, matched exactly.
    pub type_name: &'static str,
}

impl KindRule {
    /// Returns whether `record_path` starts with one of the prefixes.
    #[must_use]
    pub fn matches_path(&self, record_path: &str) -> bool {
        self.prefixes
            .iter()
            .any(|prefix| starts_with_folder(record_path, prefix))
    }

    /// Returns whether a Record with `record_path` and the struct `type_name` is selected.
    pub(crate) fn selects(&self, record_path: &str, type_name: &str) -> bool {
        self.type_name == type_name && self.matches_path(record_path)
    }
}

/// Returns whether `record_path` starts with `prefix`, ignoring ASCII case and the kind of
/// separator.
pub(crate) fn starts_with_folder(record_path: &str, prefix: &str) -> bool {
    let fold = |byte: u8| {
        if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        }
    };
    record_path.len() >= prefix.len()
        && record_path
            .bytes()
            .zip(prefix.bytes())
            .all(|(have, want)| fold(have) == fold(want))
}
