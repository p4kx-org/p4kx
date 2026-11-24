// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_archive::{Archive, Entry};
use p4kx_cryxml::{self as cryxml, Format};
use rayon::prelude::*;

use crate::coverage::{EntryCoverage, ReferenceItem, ReferenceKind};
use crate::coverage_origins::FailureSite;

/// The largest Entry the check reads into memory.
const MAX_ENTRY_LEN: u64 = 64 << 20;

enum Outcome {
    Converted,
    Text,
    Failed(String),
}

struct FailedEntry<'archive> {
    entry: &'archive Entry,
    reason: String,
}

pub(crate) fn check<'archive>(
    archive: &'archive Archive,
    items: &[ReferenceItem<'_>],
    mut sites: Option<&mut Vec<FailureSite<'archive>>>,
) -> Vec<EntryCoverage> {
    items
        .iter()
        .filter(|item| item.kind == ReferenceKind::Entries)
        .enumerate()
        .map(|(folder_index, item)| folder(archive, item, folder_index, sites.as_deref_mut()))
        .collect()
}

fn folder<'archive>(
    archive: &'archive Archive,
    item: &ReferenceItem<'_>,
    folder_index: usize,
    sites: Option<&mut Vec<FailureSite<'archive>>>,
) -> EntryCoverage {
    let inside: Vec<&Entry> = archive
        .entries()
        .iter()
        .filter(|entry| in_folder(entry.path(), item.name))
        .collect();
    let outcomes: Vec<(&Entry, Outcome)> = inside
        .par_iter()
        .map(|entry| (*entry, convert(archive, entry)))
        .collect();
    let mut coverage = EntryCoverage {
        area: item.area.to_owned(),
        folder: item.name.to_owned(),
        entries: outcomes.len() as u64,
        converted: 0,
        text: 0,
        failures: Vec::new(),
    };
    let mut failed = Vec::new();
    for (entry, outcome) in outcomes {
        match outcome {
            Outcome::Converted => coverage.converted += 1,
            Outcome::Text => coverage.text += 1,
            Outcome::Failed(reason) => failed.push(FailedEntry { entry, reason }),
        }
    }
    failed.sort_by(|left, right| {
        (left.entry.path(), &left.reason).cmp(&(right.entry.path(), &right.reason))
    });
    if let Some(sites) = sites {
        sites.extend(failed.iter().enumerate().map(|(row, failure)| FailureSite {
            folder: folder_index,
            row,
            entry: failure.entry,
        }));
    }
    coverage.failures = failed
        .into_iter()
        .map(|failure| (failure.entry.path().to_owned(), failure.reason))
        .collect();
    coverage
}

fn convert(archive: &Archive, entry: &Entry) -> Outcome {
    let size = entry.uncompressed_size();
    if size > MAX_ENTRY_LEN {
        return Outcome::Failed(format!("{size} bytes, above the limit of {MAX_ENTRY_LEN}"));
    }
    let mut bytes = Vec::new();
    let read = archive
        .open_entry(entry)
        .map_err(|error| error.to_string())
        .and_then(|reader| {
            reader
                .take(size)
                .read_to_end(&mut bytes)
                .map_err(|error| error.to_string())
        });
    if let Err(reason) = read {
        return Outcome::Failed(reason);
    }
    match cryxml::detect(&bytes) {
        Some(Format::CryXml(_)) => {}
        Some(_) => return Outcome::Text,
        None => return Outcome::Failed("neither CryXml nor text XML".to_owned()),
    }
    match cryxml::parse(&bytes) {
        Ok(document) => match document.write_xml(&mut io::sink()) {
            Ok(()) => Outcome::Converted,
            Err(error) => Outcome::Failed(error.to_string()),
        },
        Err(error) => Outcome::Failed(error.to_string()),
    }
}

/// Returns whether `path` starts with `folder`, ignoring ASCII case and the kind of separator.
fn in_folder(path: &str, folder: &str) -> bool {
    let same = |left: u8, right: u8| {
        let slash = |byte: u8| {
            if byte == b'\\' {
                b'/'
            } else {
                byte.to_ascii_lowercase()
            }
        };
        slash(left) == slash(right)
    };
    path.as_bytes().get(..folder.len()).is_some_and(|start| {
        start
            .iter()
            .zip(folder.as_bytes())
            .all(|(left, right)| same(*left, *right))
    })
}

#[cfg(test)]
mod tests {
    use super::in_folder;

    #[test]
    fn a_folder_matches_ignoring_case_and_separator_kind() {
        assert!(in_folder(
            "Data\\Scripts\\Entities\\a.xml",
            "data/scripts/entities/"
        ));
        assert!(!in_folder("Data\\Scripts", "Data/Scripts/Entities/"));
        assert!(!in_folder("Data\\Other\\a.xml", "Data/Scripts/"));
    }
}
