// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Build-bound comparison of physical Tag definitions, root arrays and selected Entity tags.

use std::error::Error;
use std::fmt;
use std::io::Write as _;
use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, ArchiveWatch, DataForgeEntry, TagTree};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

#[path = "catalog_tags/compare.rs"]
mod compare;

const MAX_RECORDS: usize = 2_000_000;

pub(super) type Result<T = ()> = std::result::Result<T, Failure>;

pub(super) struct Failure {
    stage: &'static str,
    cause: Option<Box<dyn Error + Send + Sync>>,
}

impl Failure {
    pub(super) const fn check(stage: &'static str) -> Self {
        Self { stage, cause: None }
    }

    fn source<E: Error + Send + Sync + 'static>(stage: &'static str, error: E) -> Self {
        Self {
            stage,
            cause: Some(Box::new(error)),
        }
    }
}

impl fmt::Debug for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.debug_struct("TagOracleFailure")
            .field("stage", &self.stage)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for Failure {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "tag oracle failed at {}", self.stage)
    }
}

impl Error for Failure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.cause
            .as_deref()
            .map(|error| error as &(dyn Error + 'static))
    }
}

pub(super) const fn ensure(value: bool, stage: &'static str) -> Result {
    if value {
        Ok(())
    } else {
        Err(Failure::check(stage))
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn actual_tags_roots_paths_and_selected_entity_fields_match_the_installed_build() -> Result {
    let path = real_archive_path().map_err(|error| Failure::source("archive-path", error))?;
    let key = real_archive_key().map_err(|error| Failure::source("archive-key", error))?;
    let archive = Archive::open_with_key(path, Some(key))
        .map_err(|error| Failure::source("archive-open", error))?;
    let watch = ArchiveWatch::new(&archive);
    watch
        .check()
        .map_err(|error| Failure::source("archive-change", error))?;
    let entry =
        DataForgeEntry::read(&archive).map_err(|error| Failure::source("dcb-read", error))?;
    let forge = entry
        .open()
        .map_err(|error| Failure::source("dcb-open", error))?;
    ensure(
        forge.records().len() <= MAX_RECORDS,
        "record-inventory-budget",
    )?;
    let options = CatalogOptions {
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let built = build_catalog_from(&archive, &entry, &options)
        .map_err(|error| Failure::source("catalog-build", error))?;
    let catalog: Value = serde_json::from_slice(built.json())
        .map_err(|error| Failure::source("catalog-json", error))?;
    let identity = archive
        .build()
        .ok_or_else(|| Failure::check("build-identity"))?;
    for (field, value) in [
        ("branch", identity.branch()),
        ("version", identity.version()),
        ("changeNumber", identity.change_number()),
        ("buildId", identity.build_id()),
    ] {
        ensure(!value.trim().is_empty(), "complete-build-identity")?;
        ensure(
            catalog
                .pointer(&format!("/source/build/{field}"))
                .and_then(Value::as_str)
                == Some(value),
            "catalog-build-identity",
        )?;
    }
    let counts = compare::definitions(&forge, &catalog, &watch)?;
    let entities = compare::entities(&forge, &catalog)?;
    let tree = TagTree::build(&forge);
    compare::paths(&tree, &catalog)?;
    watch
        .check()
        .map_err(|error| Failure::source("archive-change", error))?;
    let unresolved = catalog
        .pointer("/exportHealth/unresolvedReferences")
        .and_then(Value::as_array)
        .ok_or_else(|| Failure::check("reference-health"))?
        .iter()
        .filter(|row| row["kind"] == "tags" || row["targetType"] == "Tag")
        .count();
    writeln!(std::io::stderr().lock(), "Tag oracle: {} definitions, {} databases, {} sampled Entities, {} paths, {} unresolved references",
        counts.0, counts.1, entities, tree.len(), unresolved).map_err(|error| Failure::source("aggregate-report", error))?;
    Ok(())
}
