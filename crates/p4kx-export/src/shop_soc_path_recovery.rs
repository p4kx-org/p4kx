// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Entry, SourceLocation, SourceUseRole};

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture};
use crate::entry_source::EntrySource;
use crate::shop_inventory_path::{FOLDER, InventoryStem};
use crate::{CatalogError, export_archive_health, shop_inventory_name, starmap_index};

const AUTHORED_FOLDER: &str = "scripts\\shopinventories\\";

pub(super) struct Reader<'read, 'input> {
    archive: &'read Archive,
    pub(super) capture: &'read EntryCapture<'input>,
    cancel: Option<&'read CancelToken>,
}

impl<'read, 'input> Reader<'read, 'input> {
    pub(super) const fn new(
        archive: &'read Archive,
        capture: &'read EntryCapture<'input>,
        cancel: Option<&'read CancelToken>,
    ) -> Self {
        Self {
            archive,
            capture,
            cancel,
        }
    }

    pub(super) fn inventory(
        &self,
        original: &AuthoredEntryPath,
    ) -> Result<Option<InventoryStem>, CatalogError> {
        if let Some(stem) = shop_inventory_name::stem(original.as_str()) {
            return InventoryStem::captured(self.capture, original, stem).map(Some);
        }
        let Some(paths) = RepairPaths::of(original.as_str()) else {
            return Ok(None);
        };
        if export_archive_health::inventory(self.archive).is_some()
            || !is_structured_json_path(self.capture, original)?
        {
            return Ok(None);
        }
        let Some(entry) = self.unique_target(&paths)? else {
            return Ok(None);
        };
        let selected = EntrySource::outer(self.capture, self.archive, entry)?;
        let target = selected.receipt().directory_path_selector()?;
        InventoryStem::corrected(self.capture, original, paths.stem, target).map(Some)
    }

    fn unique_target(&self, paths: &RepairPaths) -> Result<Option<&Entry>, CatalogError> {
        let mut found = None;
        for entry in self.archive.entries() {
            if self.cancel.is_some_and(CancelToken::is_cancelled) {
                return Err(CatalogError::Cancelled);
            }
            let Some(name) = entry.path().rsplit(['/', '\\']).next() else {
                continue;
            };
            if let Some(kind) = paths.matches(entry.path(), name) {
                match kind {
                    Match::Target if found.is_none() => found = Some(entry),
                    Match::Target | Match::Other => return Ok(None),
                }
            }
        }
        Ok(found)
    }
}

struct RepairPaths {
    stem: String,
    original: String,
    target: String,
    alternate: String,
}

enum Match {
    Target,
    Other,
}

impl RepairPaths {
    fn of(original: &str) -> Option<Self> {
        let path = starmap_index::key(original);
        let relative = path.strip_prefix("data\\").unwrap_or(&path);
        let original = relative.strip_prefix(AUTHORED_FOLDER)?;
        let stem = original.strip_suffix(".jsonn")?;
        if stem.contains('\\') {
            return None;
        }
        let target = format!("{stem}.json");
        Some(Self {
            stem: shop_inventory_name::stem(&target)?,
            original: original.to_owned(),
            target,
            alternate: format!("{stem}.xml"),
        })
    }

    fn matches(&self, path: &str, name: &str) -> Option<Match> {
        let (file, kind) = if name.eq_ignore_ascii_case(&self.target) {
            (&self.target, Match::Target)
        } else if name.eq_ignore_ascii_case(&self.original) {
            (&self.original, Match::Other)
        } else if name.eq_ignore_ascii_case(&self.alternate) {
            (&self.alternate, Match::Other)
        } else {
            return None;
        };
        let canonical = starmap_index::key(path);
        if canonical.strip_prefix(FOLDER) == Some(file.as_str()) {
            Some(kind)
        } else if canonical.strip_prefix(AUTHORED_FOLDER) == Some(file.as_str()) {
            Some(Match::Other)
        } else {
            None
        }
    }
}

fn is_structured_json_path(
    capture: &EntryCapture<'_>,
    original: &AuthoredEntryPath,
) -> Result<bool, CatalogError> {
    let original = original.retain_in(capture)?;
    let (uses, complete) = original.contributions();
    let mut payloads = uses
        .iter()
        .filter(|used| used.role() == SourceUseRole::Payload);
    Ok(complete
        && payloads.next().is_some_and(|payload| {
            matches!(payload.description().location(), SourceLocation::Subresource {
                namespace, field, indices
            } if (namespace == "socCryXml" || namespace == "socTextXml") && indices.len() == 3
                && field.eq_ignore_ascii_case("offlineInventoryJSON"))
        })
        && payloads.next().is_none())
}
