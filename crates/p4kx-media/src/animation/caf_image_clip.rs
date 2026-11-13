// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::io;

use p4kx_extraction::model::{CafImagePath, CafImageWord};

use super::super::dba_metadata_lookup::{self, Inventory};
use super::super::image_payload::{DbaClipCandidate, DbaClipLookup};
use super::super::origin as proof;
use super::super::payload as wire;
use super::ImageSource;
use super::caf_image_bank::ObservedBank;
use crate::animation::selection_path::archive_animation_path;
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Reference, SourceContext, Subresource, required_input};

const FIELD: &str = "/data/payload/dba_clip";

pub(super) struct ImageDbaBinding<'a> {
    pub(super) entry: Option<Box<Reference>>,
    pub(super) clip: DbaClipLookup<'a>,
}

#[derive(Default)]
pub(super) struct DbaMetadataCache {
    records: HashMap<usize, Option<Inventory>>,
}

impl DbaMetadataCache {
    fn read(
        &mut self,
        context: &SourceContext<'_>,
        position: usize,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Option<&Inventory>, MediaError> {
        let directory = context.directory;
        let archive = directory.index.archive;
        let entry = archive
            .entries()
            .get(position)
            .ok_or_else(|| MediaError::Contract("resolved image DBA Entry is absent".into()))?;
        if !self.records.contains_key(&position) {
            self.records
                .try_reserve(1)
                .map_err(|cause| MediaError::Io {
                    path: context.entry.path().into(),
                    source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
                })?;
        }
        let result = match self.records.entry(position) {
            Entry::Occupied(slot) => slot.into_mut(),
            Entry::Vacant(slot) => {
                let value = if entry.is_encrypted() && !archive.has_key() {
                    None
                } else {
                    let use_at = Subresource::new(wire::IMAGE_RECORD, Vec::new());
                    Some(required_input::with_reference_input(
                        directory,
                        entry,
                        &use_at,
                        writer,
                        |input, writer| dba_metadata_lookup::read(input, context, writer),
                    )?)
                };
                slot.insert(value)
            }
        };
        Ok(result.as_ref())
    }
}

impl ImageSource<'_, '_> {
    pub(super) fn dba_binding<'a>(
        &self,
        hash: CafImageWord,
        path: CafImagePath<'_>,
        path_proof: &OriginContributions,
        metadata: &'a mut DbaMetadataCache,
        prepared: &mut PreparedRecordOrigins,
        writer: &mut MediaWriter<'_>,
    ) -> Result<ImageDbaBinding<'a>, MediaError> {
        let Some(bank) = self.dba_entry(hash, prepared, writer)? else {
            let no_bank = prepared
                .visit("/data/payload/dba_entry")
                .ok_or_else(|| MediaError::Contract("missing DBA selector proof".into()))?;
            proof::at(prepared, FIELD, no_bank.clone(), writer)?;
            return Ok(ImageDbaBinding {
                entry: None,
                clip: DbaClipLookup::NoBankDeclared,
            });
        };
        let mut receipt = writer.capture_origins();
        receipt.include(&bank.selection, writer)?;
        let binding = self.clip(&bank, path, path_proof, metadata, &mut receipt, writer)?;
        proof::at(prepared, FIELD, receipt.finish(), writer)?;
        if let DbaClipLookup::Observed { candidates, .. } = &binding {
            for (index, candidate) in candidates.iter().enumerate() {
                let source = &candidate.proof;
                for (field, evidence) in [
                    ("clip_ordinal", &source.metadata),
                    ("authored_name", &source.name),
                    ("name_range", &source.name),
                    ("metadata_range", &source.metadata),
                ] {
                    proof::at(
                        prepared,
                        &format!("{FIELD}/candidates/{index}/{field}"),
                        evidence.clone(),
                        writer,
                    )?;
                }
            }
        }
        Ok(ImageDbaBinding {
            entry: Some(bank.reference),
            clip: binding,
        })
    }

    fn clip<'a>(
        &self,
        bank: &ObservedBank,
        path: CafImagePath<'_>,
        path_proof: &OriginContributions,
        metadata: &'a mut DbaMetadataCache,
        receipt: &mut crate::origin_capture::OriginCapture,
        writer: &mut MediaWriter<'_>,
    ) -> Result<DbaClipLookup<'a>, MediaError> {
        let Some(position) = bank.position else {
            return Ok(DbaClipLookup::BankUnresolved);
        };
        receipt.include(path_proof, writer)?;
        let CafImagePath::Text(path) = path else {
            return Ok(DbaClipLookup::InvalidCafPath);
        };
        let Some(path) = path
            .rsplit_once('.')
            .filter(|(_, ext)| ext.eq_ignore_ascii_case("caf"))
            .and_then(|_| archive_animation_path(path))
        else {
            return Ok(DbaClipLookup::InvalidCafPath);
        };
        let Some(inventory) = metadata.read(self.context, position, writer)? else {
            return Ok(DbaClipLookup::BankUnreadable);
        };
        receipt.include(&inventory.coverage, writer)?;
        self.candidates(inventory, &path, receipt, writer)
    }

    fn candidates<'a>(
        &self,
        inventory: &'a Inventory,
        path: &str,
        receipt: &mut crate::origin_capture::OriginCapture,
        writer: &MediaWriter<'_>,
    ) -> Result<DbaClipLookup<'a>, MediaError> {
        let mut candidates = Vec::new();
        let cap = writer.limits().reference_candidates as usize;
        let mut observed_candidates = 0;
        for matched in inventory.matches(&path.to_ascii_lowercase()) {
            writer.checkpoint()?;
            observed_candidates += 1;
            if candidates.len() >= cap {
                continue;
            }
            let evidence = matched.proof(writer)?;
            receipt.include(&evidence.name, writer)?;
            candidates.try_reserve(1).map_err(|cause| MediaError::Io {
                path: self.context.entry.path().into(),
                source: io::Error::new(io::ErrorKind::OutOfMemory, cause),
            })?;
            candidates.push(DbaClipCandidate {
                clip_ordinal: matched.record.ordinal,
                authored_name: &matched.record.authored_name,
                name_range: wire::entry_range(matched.record.name.clone()),
                metadata_range: wire::entry_range(matched.record.metadata.clone()),
                proof: evidence,
            });
        }
        Ok(DbaClipLookup::Observed {
            candidates_complete: inventory.complete && candidates.len() == observed_candidates,
            observed_candidates,
            candidates,
        })
    }
}
