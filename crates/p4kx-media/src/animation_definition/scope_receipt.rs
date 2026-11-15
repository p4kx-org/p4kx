// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    EntityRig, ModelRig, RigDefinition, RigFile, RigFileRead, RigParameter, RigPlacement,
};
use p4kx_extraction::{DecimalU64, Entry, InputScope, SourceLocation, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use super::scope_placement::{PlacementDetail, PlacementEvidence};
use crate::audio_record_origins as records;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{
    CandidateInventory, DirectoryLookup, MediaError, MediaWriter, Status, Subresource, origin_error,
};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ScopeReceipt<'source> {
    input_entry_index: DecimalU64,
    placement: ScopePlacement,
    #[serde(skip_serializing_if = "Option::is_none")]
    placement_detail: Option<PlacementDetail<'source>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    owner_record_index: Option<u32>,
    outcome: ScopeOutcome,
}

#[derive(Serialize, JsonSchema, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
pub(super) enum ScopePlacement {
    Model,
    ExcludedEntityInput,
    EntityRoot,
    Attachment,
    LandingGear,
    Other,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
enum ScopeOutcome {
    Selected { entry_index: DecimalU64 },
    Excluded { entry_index: DecimalU64 },
    NoCompanion,
    NoRig,
}

impl ScopeOutcome {
    fn of(
        definition: Option<&RigFile<RigDefinition>>,
        parameters: Option<&RigFile<Vec<RigParameter>>>,
        inventory: CandidateInventory,
    ) -> Result<Self, MediaError> {
        if let Some(parameters) = parameters {
            return match parameters.read.as_ref() {
                RigFileRead::Read { value: Ok(_), .. } => Ok(Self::Selected {
                    entry_index: index(parameters)?,
                }),
                RigFileRead::NotSelected => Self::excluded(parameters),
                RigFileRead::Missing if matches!(inventory, CandidateInventory::Complete) => {
                    Ok(Self::NoCompanion)
                }
                RigFileRead::Missing => Err(MediaError::Contract(
                    "animation companion absence requires a complete directory inventory".into(),
                )),
                _ => Err(MediaError::Contract(
                    "animation scope admitted an unverified parameter input".into(),
                )),
            };
        }
        match definition {
            Some(definition) if matches!(definition.read.as_ref(), RigFileRead::NotSelected) => {
                Self::excluded(definition)
            }
            _ => Ok(Self::NoRig),
        }
    }

    fn excluded<T>(file: &RigFile<T>) -> Result<Self, MediaError> {
        Ok(Self::Excluded {
            entry_index: index(file)?,
        })
    }

    const fn entry(&self) -> Option<u64> {
        match self {
            Self::Selected { entry_index } | Self::Excluded { entry_index } => {
                Some(entry_index.get())
            }
            _ => None,
        }
    }
}

fn index<T>(file: &RigFile<T>) -> Result<DecimalU64, MediaError> {
    file.entry_index()
        .map(|index| DecimalU64::new(index as u64))
        .ok_or_else(origin_error::foreign)
}

pub(super) fn excluded(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    placement: ScopePlacement,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let scope = Scope::new(lookup, entry, writer)?;
    let entry_index = DecimalU64::new(entry.index() as u64);
    scope.write(
        0,
        placement,
        None,
        ScopeOutcome::Excluded { entry_index },
        scope.entry.clone(),
        writer,
    )
}

pub(super) fn model(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    rig: &ModelRig,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let scope = Scope::new(lookup, entry, writer)?;
    let outcome = ScopeOutcome::of(
        rig.definition.as_ref(),
        rig.parameters.as_ref(),
        lookup.index.inventory(),
    )?;
    let proof = scope.decision(&outcome, rig.definition.as_ref(), &scope.entry, writer)?;
    scope.write(0, ScopePlacement::Model, None, outcome, proof, writer)
}

pub(super) fn entity(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    rigs: &[EntityRig],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut scope = Scope::new(lookup, entry, writer)?;
    let input = writer.origins.input(lookup.index.archive, entry)?;
    scope.placement = payload(&input, entry.uncompressed_size(), writer)?;
    for (ordinal, rig) in rigs.iter().enumerate() {
        let placement = match &rig.placement {
            RigPlacement::Root => ScopePlacement::EntityRoot,
            RigPlacement::Attachment { .. } => ScopePlacement::Attachment,
            RigPlacement::LandingGear { .. } => ScopePlacement::LandingGear,
            _ => ScopePlacement::Other,
        };
        let outcome = ScopeOutcome::of(
            rig.definition.as_ref(),
            rig.parameters.as_ref(),
            lookup.index.inventory(),
        )?;
        let proof = scope.decision(&outcome, rig.definition.as_ref(), &scope.placement, writer)?;
        scope.write(
            ordinal as u64,
            placement,
            Some(PlacementEvidence { rig, input: &input }),
            outcome,
            proof,
            writer,
        )?;
    }
    Ok(())
}

struct Scope<'a, 'directory> {
    lookup: &'a DirectoryLookup<'directory>,
    owner: MediaOriginInput,
    input_index: u64,
    entry: OriginContributions,
    placement: OriginContributions,
}

impl<'a, 'directory> Scope<'a, 'directory> {
    fn new(
        lookup: &'a DirectoryLookup<'directory>,
        entry: &Entry,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let owner = writer
            .origins
            .directory()
            .cloned()
            .ok_or_else(origin_error::foreign)?;
        let input_index = entry.index() as u64;
        let entry = origins::directory_entry(&owner, input_index, writer)?;
        Ok(Self {
            lookup,
            owner,
            input_index,
            placement: entry.clone(),
            entry,
        })
    }

    fn decision(
        &self,
        outcome: &ScopeOutcome,
        definition: Option<&RigFile<RigDefinition>>,
        basis: &OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let mut proof = writer.capture_origins();
        proof.include(basis, writer)?;
        if let Some(index) = outcome.entry() {
            proof.include(
                &origins::directory_entry(&self.owner, index, writer)?,
                writer,
            )?;
        }
        if let ScopeOutcome::Selected { entry_index } = outcome {
            let index = usize::try_from(entry_index.get()).map_err(|_| origin_error::counter())?;
            let entry = self.physical_entry(index)?;
            let source = writer.origins.input(self.lookup.index.archive, entry)?;
            proof.include(
                &payload(&source, entry.uncompressed_size(), writer)?,
                writer,
            )?;
        }
        if matches!(outcome, ScopeOutcome::NoCompanion) {
            let inventory = crate::origin_lookup::directory(Some(&self.owner), writer)?;
            proof.include(&inventory, writer)?;
        }
        if let Some(definition) = definition {
            proof.include(&self.cdf_selector(definition, writer)?, writer)?;
        }
        Ok(proof.finish())
    }

    fn cdf_selector(
        &self,
        definition: &RigFile<RigDefinition>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let Some(source) = definition.source() else {
            return Ok(OriginContributions::independent(&writer.origins.owner));
        };
        let entry = self.physical_entry(source.entry().index())?;
        let input = writer.origins.input(self.lookup.index.archive, entry)?;
        let mut proof = writer.capture_origins();
        match definition.value().and_then(|value| value.selector.as_ref()) {
            Some(SourceLocation::Subresource {
                namespace,
                indices,
                field,
            }) => proof.add(
                &input,
                InputScope::Payload,
                OriginSite::new(namespace, indices, field),
                SourceUseRole::Selector,
                writer,
            )?,
            _ => proof.include(&payload(&input, entry.uncompressed_size(), writer)?, writer)?,
        }
        Ok(proof.finish())
    }

    fn physical_entry(&self, position: usize) -> Result<&Entry, MediaError> {
        self.lookup
            .index
            .archive
            .entries()
            .get(position)
            .ok_or_else(origin_error::foreign)
    }

    fn write(
        &self,
        ordinal: u64,
        placement: ScopePlacement,
        evidence: Option<PlacementEvidence<'_>>,
        outcome: ScopeOutcome,
        proof: OriginContributions,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = origins::record(&self.owner, self.placement.clone(), writer)?;
        records::field(
            &mut prepared,
            "/data/payload/input_entry_index",
            self.entry.clone(),
            writer,
        )?;
        records::subtree(&mut prepared, "/data/payload/outcome", proof, writer)?;
        if let Some(index) = outcome.entry() {
            records::field(
                &mut prepared,
                "/data/payload/outcome/entry_index",
                origins::directory_entry(&self.owner, index, writer)?,
                writer,
            )?;
        }
        let owner_record_index = evidence
            .as_ref()
            .and_then(|evidence| evidence.rig.record_index);
        if let Some((evidence, record)) = evidence.as_ref().zip(owner_record_index) {
            let mut proof = writer.capture_origins();
            proof.add(
                evidence.input,
                InputScope::Payload,
                OriginSite::new("dataforge-record", &[u64::from(record)], "recordIndex"),
                SourceUseRole::Selector,
                writer,
            )?;
            records::field(
                &mut prepared,
                "/data/payload/owner_record_index",
                proof.finish(),
                writer,
            )?;
        }
        let placement_detail = match evidence {
            Some(evidence) => evidence.write(&mut prepared, writer)?,
            None => None,
        };
        prepared.write(
            writer,
            &Subresource::new(wire::SCOPE_RECEIPT, vec![ordinal]),
            Status::Complete,
            &DefinitionPayload::ScopeReceipt(ScopeReceipt {
                input_entry_index: DecimalU64::new(self.input_index),
                placement,
                placement_detail,
                owner_record_index,
                outcome,
            }),
            &[],
        )
    }
}

fn payload(
    input: &MediaOriginInput,
    size: u64,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    records::capture(
        input,
        [(
            OriginSite::new("media-physical-input", &[0, size], "complete-payload"),
            SourceUseRole::Selector,
        )],
        writer,
    )
}
