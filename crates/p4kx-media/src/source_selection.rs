// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Exclusion, Filter};

use crate::ContainerIdentity;

#[derive(Clone, Copy)]
pub(crate) struct SourceSelection<'a> {
    pub(crate) include: &'a Filter,
    pub(crate) exclude: &'a Exclusion,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EntrySelection {
    Included,
    Unmatched,
    Excluded,
}

impl SourceSelection<'_> {
    pub(crate) fn classify(&self, chain: &[ContainerIdentity], path: &str) -> EntrySelection {
        let qualified = qualified_path(chain, path);
        if self.excluded(path) || self.exclude.matches(&qualified) {
            EntrySelection::Excluded
        } else if self.include.matches(path) || self.include.matches(&qualified) {
            EntrySelection::Included
        } else {
            EntrySelection::Unmatched
        }
    }

    fn excluded(&self, path: &str) -> bool {
        if self.exclude.matches(path) {
            return true;
        }
        let lower = path.to_ascii_lowercase();
        if let Some(end) = lower.find(".dds.") {
            return self.exclude.matches(path.get(..end + 4).unwrap_or(path));
        }
        false
    }
}

fn qualified_path(chain: &[ContainerIdentity], path: &str) -> String {
    let mut result = String::new();
    for container in chain {
        result.push_str(&container.path);
        result.push('!');
    }
    result.push_str(path);
    result
}
