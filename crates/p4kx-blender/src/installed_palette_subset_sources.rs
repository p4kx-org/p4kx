// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::convert::Infallible;
use std::io::{self, Read as _};

use p4kx_extraction::{Entry, PathIndex};
use serde::Serialize;
use serde_json::Value;

use super::Check;
use super::input::{Request, SelectedEntry as BoundEntry, canonical};
use super::output::ExportedSubset;

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum ReadRole {
    PaletteDatabase,
    LayerMaterial,
    WrapperMaterial,
    GeometryChecksumReplay,
    NormalPackageInputChecksumReplay,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VerifiedRead {
    role: ReadRole,
    entry_index: usize,
    path: String,
    checksum_kind: &'static str,
    checksum: String,
    verified_decoded_bytes: u64,
    checksum_verified_to_eof: bool,
    verification_pass: u64,
    source_verification_pass: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct VerificationCounts {
    actual_verified_streams: u64,
    unique_verified_sources: u64,
    verified_decoded_bytes: u64,
    reused_verification_results: u64,
}

#[derive(Default)]
pub(super) struct Ledger {
    pub(super) rows: Vec<VerifiedRead>,
    checked: BTreeMap<usize, u64>,
    verified_bytes: u64,
    reused: u64,
}

pub(super) fn entry<'a>(index: &PathIndex<'a>, bound: &BoundEntry) -> Check<&'a Entry> {
    let mut found = index.matching_entries_with(&bound.path, |_| Ok::<(), Infallible>(()))?;
    let first = found
        .next()
        .transpose()?
        .ok_or("the bound physical Entry is absent")?;
    if found.next().transpose()?.is_some()
        || first.index() != bound.entry_index
        || first.uncompressed_size() != bound.decoded_bytes
    {
        return Err("the bound Entry is ambiguous or its source size/ordinal differs".into());
    }
    Ok(first)
}

impl Ledger {
    pub(super) fn read(
        &mut self,
        index: &PathIndex<'_>,
        bound: &BoundEntry,
        role: ReadRole,
    ) -> Check<Vec<u8>> {
        let source = entry(index, bound)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(usize::try_from(bound.decoded_bytes)?)?;
        index
            .archive()
            .open_entry_verified(source)?
            .read_to_end(&mut bytes)?;
        if u64::try_from(bytes.len())? != bound.decoded_bytes {
            return Err("the verified original payload length differs".into());
        }
        self.record(source, role, u64::try_from(bytes.len())?)?;
        Ok(bytes)
    }

    pub(super) fn verify(
        &mut self,
        index: &PathIndex<'_>,
        bound: &BoundEntry,
        role: ReadRole,
    ) -> Check {
        if self.checked.contains_key(&bound.entry_index) {
            self.reused = self
                .reused
                .checked_add(1)
                .ok_or("verification reuse count overflow")?;
            return Ok(());
        }
        let source = entry(index, bound)?;
        let bytes = io::copy(
            &mut index.archive().open_entry_verified(source)?,
            &mut io::sink(),
        )?;
        if bytes != bound.decoded_bytes {
            return Err("the verified source size differs".into());
        }
        self.record(source, role, bytes)
    }

    fn record(&mut self, source: &Entry, role: ReadRole, bytes: u64) -> Check {
        let source_pass = self.checked.entry(source.index()).or_default();
        *source_pass = source_pass
            .checked_add(1)
            .ok_or("source verification count overflow")?;
        self.verified_bytes = self
            .verified_bytes
            .checked_add(bytes)
            .ok_or("verification byte count overflow")?;
        let pass = u64::try_from(self.rows.len())?
            .checked_add(1)
            .ok_or("verification stream count overflow")?;
        self.rows.push(VerifiedRead {
            role,
            entry_index: source.index(),
            path: source.path().to_owned(),
            checksum_kind: source.checksum().name(),
            checksum: source.checksum().hex(),
            verified_decoded_bytes: bytes,
            checksum_verified_to_eof: true,
            verification_pass: pass,
            source_verification_pass: *source_pass,
        });
        Ok(())
    }

    pub(super) fn counts(&self) -> Check<VerificationCounts> {
        Ok(VerificationCounts {
            actual_verified_streams: u64::try_from(self.rows.len())?,
            unique_verified_sources: u64::try_from(self.checked.len())?,
            verified_decoded_bytes: self.verified_bytes,
            reused_verification_results: self.reused,
        })
    }
}

pub(super) fn verify_output_sources(
    index: &PathIndex<'_>,
    request: &Request,
    result: &ExportedSubset,
    ledger: &mut Ledger,
) -> Check {
    let mut actual = BTreeSet::new();
    actual_entries(&serde_json::to_value(&result.manifest)?, &mut actual)?;
    for ordinal in actual {
        let bound = request
            .allowed_sources
            .iter()
            .find(|row| row.entry_index == ordinal)
            .ok_or("a producer read lies outside the prepared physical closure")?;
        ledger.verify(index, bound, ReadRole::NormalPackageInputChecksumReplay)?;
    }
    let package_paths: BTreeSet<_> = result
        .manifest
        .textures
        .iter()
        .map(|row| canonical(&row.source))
        .collect();
    if result
        .requests
        .iter()
        .any(|(path, mip)| *mip != request.mip || !package_paths.contains(&canonical(path)))
    {
        return Err(
            "a GLB input is absent from the ordinary package's original input closure".into(),
        );
    }
    Ok(())
}

fn actual_entries(value: &Value, ordinals: &mut BTreeSet<usize>) -> Check {
    match value {
        Value::Object(fields) => {
            if fields.get("kind").and_then(Value::as_str) == Some("archiveEntry") {
                let ordinal = fields
                    .get("entryIndex")
                    .and_then(Value::as_u64)
                    .ok_or("an original physical source ordinal")?;
                ordinals.insert(usize::try_from(ordinal)?);
            }
            for child in fields.values() {
                actual_entries(child, ordinals)?;
            }
        }
        Value::Array(values) => {
            for child in values {
                actual_entries(child, ordinals)?;
            }
        }
        _ => {}
    }
    Ok(())
}
