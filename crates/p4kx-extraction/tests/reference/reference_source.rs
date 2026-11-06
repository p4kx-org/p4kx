// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{Archive, ArchiveWatch, Entry, Guid};

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_identity::{self, LogicalKey};
use super::reference_manifest::{
    ArtifactRole, BuildIdentity, Identity, MAX_ARTIFACT_BYTES, Manifest, SelectionMode,
    SourceDataset, Tool,
};
use super::reference_paths;

pub(crate) struct MeasuredSource {
    pub(crate) dataset: SourceDataset,
    pub(crate) entries: BTreeMap<String, usize>,
    records: BTreeSet<Guid>,
    universe: BTreeSet<LogicalKey>,
}

fn build(archive: &Archive) -> Result<BuildIdentity> {
    let build = archive.build().ok_or_else(|| {
        ReferenceError::new(Code::Provenance, "source Archive has no Build identity")
    })?;
    let channel = archive
        .path()
        .parent()
        .and_then(|path| path.file_name())
        .and_then(|name| name.to_str())
        .ok_or_else(|| ReferenceError::new(Code::Provenance, "source Channel is not identified"))?;
    Ok(BuildIdentity {
        channel: channel.to_owned(),
        branch: build.branch().to_owned(),
        version: build.version().to_owned(),
        change_number: build.change_number().to_owned(),
        build_id: build.build_id().to_owned(),
    })
}

pub(crate) fn measure(archive: &Archive, tool: Tool) -> Result<MeasuredSource> {
    let watch = ArchiveWatch::new(archive);
    watch
        .check()
        .map_err(|error| ReferenceError::new(Code::Changed, error.to_string()))?;
    let entries = super::reference_inventory::entries(archive)?;
    let dataset = SourceDataset {
        archive: reference_digest::file(archive.path(), u64::MAX)?,
        build: build(archive)?,
        inventory: super::reference_inventory::fingerprint(archive, &entries)?,
        dcb: None,
    };
    let mut source = MeasuredSource {
        dataset,
        entries,
        records: BTreeSet::new(),
        universe: BTreeSet::new(),
    };
    if tool == Tool::Starbreaker {
        let records = super::reference_records::measure(archive)?;
        source.dataset.dcb = Some(records.dcb);
        source.records = records.guids;
        source.universe = records.universe;
    } else {
        source.universe = source
            .entries
            .keys()
            .filter(|path| {
                tool == Tool::Unp4k
                    || std::path::Path::new(path)
                        .extension()
                        .is_some_and(|extension| {
                            extension.eq_ignore_ascii_case("cdf")
                                || extension.eq_ignore_ascii_case("chrparams")
                        })
            })
            .cloned()
            .map(LogicalKey::Entry)
            .collect();
    }
    watch
        .check()
        .map_err(|error| ReferenceError::new(Code::Changed, error.to_string()))?;
    Ok(source)
}

impl MeasuredSource {
    pub(crate) fn validate(&self, manifest: &Manifest) -> Result<()> {
        if self.dataset != manifest.source {
            return Err(ReferenceError::new(
                Code::Provenance,
                "Archive/Build/inventory/DCB fingerprint mismatch",
            ));
        }
        if manifest.selection.mode == SelectionMode::MainOnly
            && manifest.producer.tool != Tool::Starbreaker
        {
            return Err(ReferenceError::new(
                Code::Selection,
                "main-only is a Record selection mode",
            ));
        }
        let mut selected = BTreeSet::new();
        for expected in &manifest.expected {
            let key = reference_identity::key(&expected.identity)?;
            match &key {
                LogicalKey::Entry(path) if self.entries.contains_key(path) => {}
                LogicalKey::Record { guid, dcb }
                    if self.records.contains(guid)
                        && self.dataset.dcb.as_ref().is_some_and(|source| {
                            reference_paths::key(&source.entry).is_ok_and(|path| path == *dcb)
                        }) => {}
                _ => {
                    return Err(ReferenceError::new(
                        Code::Identity,
                        "expected logical source is not in source dataset",
                    ));
                }
            }
            selected.insert(key);
        }
        if manifest.selection.mode == SelectionMode::All && selected != self.universe {
            return Err(ReferenceError::new(
                Code::Coverage,
                "all-source selection omits or invents source identities",
            ));
        }
        Ok(())
    }

    pub(crate) fn entry<'a>(&self, archive: &'a Archive, identity: &Identity) -> Result<&'a Entry> {
        let LogicalKey::Entry(path) = reference_identity::key(identity)? else {
            return Err(ReferenceError::new(Code::Identity, "not an Entry identity"));
        };
        self.entries
            .get(&path)
            .and_then(|index| archive.entries().get(*index))
            .ok_or_else(|| ReferenceError::new(Code::Identity, "source Entry is absent"))
    }

    pub(crate) fn verify_source_copies(
        &self,
        archive: &Archive,
        root: &std::path::Path,
        manifest: &Manifest,
    ) -> Result<()> {
        for artifact in &manifest.artifacts {
            if let ArtifactRole::SourceCopy { identity } = &artifact.role {
                let entry = self.entry(archive, identity)?;
                let reader = archive
                    .open_entry(entry)
                    .map_err(|error| ReferenceError::new(Code::Provenance, error.to_string()))?;
                let actual = reference_digest::reader(reader, MAX_ARTIFACT_BYTES)?;
                let path = reference_paths::resolve(root, &artifact.path)?;
                if reference_digest::file(&path, MAX_ARTIFACT_BYTES)? != actual {
                    return Err(ReferenceError::new(
                        Code::Provenance,
                        "source-copy artifact is not the declared source Entry",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn universe_len(&self) -> usize {
        self.universe.len()
    }
}
