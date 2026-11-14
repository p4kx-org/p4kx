// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{EntityClipCatalogue, EntityRig, RigFile, RigFileRead, RigPlacement};
use p4kx_extraction::{InputScope, SourceLocation, SourceUseRole};

use super::super::db;
use super::{
    Attempt, Owner, PARAMETER, Parameter, Payload, Placement, RIG, ReadStatus, Rig, Scope,
};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::{DirectoryLookup, MediaError, MediaWriter};

#[path = "entity_catalogue_banks.rs"]
mod banks;

#[path = "entity_catalogue_links.rs"]
mod links;

#[path = "entity_catalogue_sources.rs"]
mod sources;

pub(in super::super) struct Rows<'a, 'dir> {
    pub(in super::super) lookup: &'a DirectoryLookup<'dir>,
    pub(in super::super) input: &'a MediaOriginInput,
}

impl Rows<'_, '_> {
    pub(in super::super) fn rigs(
        &self,
        catalogue: &EntityClipCatalogue,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut banks = banks::Banks::default();
        for (position, rig) in catalogue.rigs.iter().enumerate() {
            self.rig(position, &rig.rig, writer)?;
            self.links(position, rig, &catalogue.sources, &mut banks, writer)?;
        }
        Ok(())
    }

    fn rig(
        &self,
        position: usize,
        rig: &EntityRig,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut scope = Scope::new(writer);
        if let Some(record) = rig.record_index {
            scope.at(
                "owner/guid",
                db(self.input, record, "guid", writer)?,
                writer,
            )?;
            scope.at(
                "owner/record",
                db(self.input, record, "recordIndex", writer)?,
                writer,
            )?;
        }
        if rig.geometry.is_some() {
            scope.at(
                "geometry",
                positions(self.input, &rig.geometry_selectors, writer)?,
                writer,
            )?;
        }
        let definition = attempt(rig.definition.as_ref());
        let mut skeleton = attempt(rig.skeleton.as_ref());
        if let Some((value, original)) = skeleton.as_mut().zip(rig.skeleton.as_ref()) {
            value.primary_chunk = original
                .value()
                .and_then(Option::as_ref)
                .map(p4kx_extraction::model::SkeletonSource::chunk_index);
            if let (Some(source), Some(chunk)) = (original.source(), value.primary_chunk) {
                let source = self.observe(source, writer)?;
                scope.at(
                    "skeleton/primary_chunk",
                    capture(
                        &source,
                        InputScope::Payload,
                        OriginSite::new("model-chunk-table", &[chunk as u64], "primarySkeleton"),
                        writer,
                    )?,
                    writer,
                )?;
            }
        }
        let parameters = attempt(rig.parameters.as_ref());
        scope.write(
            RIG,
            vec![position as u64],
            &Payload::Rig(Rig {
                owner: Owner {
                    guid: rig.owner.to_string(),
                    record: rig.record_index,
                },
                placement: placement(&rig.placement),
                geometry: rig.geometry.as_deref(),
                definition,
                skeleton,
                parameters,
            }),
            &[],
            writer,
        )
    }

    fn parameter(
        rig: usize,
        slot: usize,
        row: &p4kx_assembly::RigParameter,
        input: Option<&MediaOriginInput>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut scope = Scope::new(writer);
        if let Some(input) = input {
            let whole = positions(input, &row.selectors, writer)?;
            for member in ["list", "ordinal", "positions"] {
                scope.at(member, whole.clone(), writer)?;
            }
            for (member, location) in ["name", "path"].into_iter().zip(&row.selectors) {
                scope.at(
                    member,
                    positions(input, std::slice::from_ref(location), writer)?,
                    writer,
                )?;
            }
        }
        scope.write(
            PARAMETER,
            vec![rig as u64, slot as u64],
            &Payload::Parameter(Parameter {
                list: row.list,
                ordinal: row.ordinal,
                name: &row.name,
                path: &row.path,
                positions: &row.selectors,
            }),
            &[],
            writer,
        )
    }

    fn observe(
        &self,
        source: &p4kx_extraction::EntryOrigin,
        writer: &MediaWriter<'_>,
    ) -> Result<MediaOriginInput, MediaError> {
        let archive = self.lookup.index.archive;
        let entry = archive
            .entries()
            .get(source.entry().index())
            .filter(|entry| entry.path() == source.entry().path())
            .ok_or_else(crate::origin_error::foreign)?;
        writer.origins.input(archive, entry)
    }
}

fn placement(placement: &RigPlacement) -> Placement<'_> {
    match placement {
        RigPlacement::Root => Placement::Root,
        RigPlacement::Attachment { port_path } => Placement::Attachment { port_path },
        RigPlacement::LandingGear {
            owner_path,
            ordinal,
            bone,
        } => Placement::LandingGear {
            owner_path,
            ordinal: *ordinal,
            bone: bone.as_deref(),
        },
        _ => Placement::Unknown,
    }
}

fn attempt<T, E>(file: Option<&RigFile<T, E>>) -> Option<Attempt<'_>> {
    let file = file?;
    Some(Attempt {
        path: &file.path,
        status: read_status(file),
        primary_chunk: None,
    })
}

fn read_status<T, E>(file: &RigFile<T, E>) -> ReadStatus {
    match file.read.as_ref() {
        RigFileRead::Missing => ReadStatus::Missing,
        RigFileRead::Ambiguous => ReadStatus::Ambiguous,
        RigFileRead::NotSelected => ReadStatus::NotSelected,
        RigFileRead::KeyRequired => ReadStatus::KeyRequired,
        RigFileRead::WrongKey => ReadStatus::WrongKey,
        RigFileRead::Read { value: Ok(_), .. } => ReadStatus::Read,
        RigFileRead::Read { value: Err(_), .. } => ReadStatus::Malformed,
        _ => ReadStatus::Unreadable,
    }
}

fn positions(
    input: &MediaOriginInput,
    locations: &[SourceLocation],
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    if locations.is_empty() {
        return Ok(OriginContributions::unknown(&writer.origins.owner));
    }
    let mut proof = writer.capture_origins();
    for location in locations {
        match location {
            SourceLocation::Subresource {
                namespace,
                indices,
                field,
            } => proof.add(
                input,
                InputScope::Payload,
                OriginSite::new(namespace, indices, field),
                SourceUseRole::Selector,
                writer,
            )?,
            SourceLocation::DataForge {
                record_index,
                field,
            } => proof.add(
                input,
                InputScope::Payload,
                OriginSite::new("dataforge-record", &[u64::from(*record_index)], field),
                SourceUseRole::Selector,
                writer,
            )?,
            _ => return Ok(OriginContributions::unknown(&writer.origins.owner)),
        }
    }
    Ok(proof.finish())
}

fn capture(
    input: &MediaOriginInput,
    scope: InputScope,
    site: OriginSite<'_>,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut proof = writer.capture_origins();
    proof.add(input, scope, site, SourceUseRole::Payload, writer)?;
    Ok(proof.finish())
}
