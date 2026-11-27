// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::{HashMap, HashSet, TryReserveError};
use std::io;

use p4kx_extraction::{DecimalU64, InputScope, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use super::origins;
use super::payload::{self as wire, DefinitionPayload};
use crate::audio_record_origins as records;
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{
    DirectoryContext, Identity, MediaError, MediaWriter, Reference, ReferenceResolution, Status,
    Subresource, error,
};

#[derive(Default)]
pub(crate) struct EventBindings(RefCell<Observed>);

#[derive(Default)]
struct Observed {
    complete: HashSet<usize>,
    aliases: Vec<Alias>,
    databases: HashMap<List, Vec<Database>>,
    timelines: HashMap<usize, HashMap<String, Vec<Timeline>>>,
}

#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub(super) struct List {
    pub(super) entry: usize,
    pub(super) element: u64,
}

struct Alias {
    list: List,
    indices: [u64; 2],
    path_key: String,
    proof: OriginContributions,
}

struct Database {
    entry: usize,
    indices: [u64; 2],
    original: OriginContributions,
    resolution: OriginContributions,
}

struct Timeline {
    element: u64,
    proof: OriginContributions,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct EventBinding {
    parameter_entry_index: DecimalU64,
    alias_element_index: DecimalU64,
    alias_declaration_ordinal: DecimalU64,
    database_element_index: DecimalU64,
    database_declaration_ordinal: DecimalU64,
    event_database_entry_index: DecimalU64,
    timeline_element_index: DecimalU64,
}

impl EventBindings {
    pub(super) fn complete(
        &self,
        entry: usize,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        writer.checkpoint()?;
        let mut observed = self.0.try_borrow_mut().map_err(|_| borrowed())?;
        observed
            .complete
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        if !observed.complete.insert(entry) {
            return Err(MediaError::Contract(
                "duplicate complete animation definition Entry".into(),
            ));
        }
        Ok(())
    }

    pub(super) fn alias(
        &self,
        list: List,
        indices: [u64; 2],
        archive_path: &str,
        proof: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        writer.checkpoint()?;
        let mut observed = self.0.try_borrow_mut().map_err(|_| borrowed())?;
        observed
            .aliases
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        observed.aliases.push(Alias {
            list,
            indices,
            path_key: archive_path.to_ascii_lowercase(),
            proof,
        });
        Ok(())
    }

    pub(super) fn database(
        &self,
        list: List,
        indices: [u64; 2],
        reference: &Reference,
        prepared: &PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let ReferenceResolution::Resolved { target } = reference.resolution() else {
            return Ok(());
        };
        let Identity::Entry { entry, .. } = &target.source else {
            return Err(MediaError::Contract(
                "animation event database is not a physical Entry".into(),
            ));
        };
        writer.checkpoint()?;
        let original = origin(prepared, "/data/payload/authored_path")?.clone();
        let resolution = origin(prepared, "/data/payload/reference/resolution")?.clone();
        let mut observed = self.0.try_borrow_mut().map_err(|_| borrowed())?;
        observed
            .databases
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        let list = observed.databases.entry(list).or_default();
        list.try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        list.push(Database {
            entry: entry.entry_index,
            indices,
            original,
            resolution,
        });
        Ok(())
    }

    pub(super) fn timeline(
        &self,
        entry: usize,
        element: u64,
        archive_path: &str,
        proof: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        writer.checkpoint()?;
        let mut observed = self.0.try_borrow_mut().map_err(|_| borrowed())?;
        observed
            .timelines
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        let paths = observed.timelines.entry(entry).or_default();
        paths
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        let occurrences = paths.entry(archive_path.to_ascii_lowercase()).or_default();
        occurrences
            .try_reserve(1)
            .map_err(|cause| allocation(writer, cause))?;
        occurrences.push(Timeline { element, proof });
        Ok(())
    }

    pub(super) fn write(
        &self,
        context: &DirectoryContext<'_, '_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let observed = self.0.try_borrow().map_err(|_| borrowed())?;
        let owner = writer.origins.directory().cloned().ok_or_else(|| {
            MediaError::Contract("animation event binding has no owning Archive".into())
        })?;
        let budget = context.lookup.input_state()?.budget;
        for alias in &observed.aliases {
            budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
            if !observed.complete.contains(&alias.list.entry) {
                continue;
            }
            let Some(databases) = observed.databases.get(&alias.list) else {
                continue;
            };
            for database in databases {
                budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
                if !observed.complete.contains(&database.entry) {
                    continue;
                }
                let Some(timelines) = observed
                    .timelines
                    .get(&database.entry)
                    .and_then(|paths| paths.get(&alias.path_key))
                else {
                    continue;
                };
                for timeline in timelines {
                    budget.metadata(0, 1, writer.limits(), writer.checkpoint)?;
                    write_binding(&owner, alias, database, timeline, writer)?;
                }
            }
        }
        Ok(())
    }
}

fn write_binding(
    owner: &MediaOriginInput,
    alias: &Alias,
    database: &Database,
    timeline: &Timeline,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut capture = writer.capture_origins();
    for proof in [&alias.proof, &database.resolution, &timeline.proof] {
        capture.include(proof, writer)?;
    }
    let mut prepared = origins::record(owner, capture.finish(), writer)?;
    let parameter_entry = alias.list.entry as u64;
    let database_entry = database.entry as u64;
    let mut entry = writer.capture_origins();
    entry.add(
        owner,
        InputScope::ArchiveDirectoryMetadata,
        OriginSite::new("archive-entry", &[parameter_entry], "entryIndex"),
        SourceUseRole::Selector,
        writer,
    )?;
    proof_fields(
        &mut prepared,
        alias,
        database,
        timeline,
        &entry.finish(),
        writer,
    )?;
    let [alias_element, alias_declaration] = alias.indices;
    let [database_element, database_declaration] = database.indices;
    let addresses = [
        parameter_entry,
        alias_element,
        alias_declaration,
        database_element,
        database_declaration,
        database_entry,
        timeline.element,
    ];
    prepared.write(
        writer,
        &Subresource::new(wire::EVENT_BINDING, addresses.to_vec()),
        Status::Complete,
        &DefinitionPayload::EventBinding(EventBinding {
            parameter_entry_index: DecimalU64::new(parameter_entry),
            alias_element_index: DecimalU64::new(alias_element),
            alias_declaration_ordinal: DecimalU64::new(alias_declaration),
            database_element_index: DecimalU64::new(database_element),
            database_declaration_ordinal: DecimalU64::new(database_declaration),
            event_database_entry_index: DecimalU64::new(database_entry),
            timeline_element_index: DecimalU64::new(timeline.element),
        }),
        &[],
    )
}

fn proof_fields(
    prepared: &mut PreparedRecordOrigins,
    alias: &Alias,
    database: &Database,
    timeline: &Timeline,
    parameter_origin: &OriginContributions,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (field, proof) in [
        ("alias_element_index", &alias.proof),
        ("alias_declaration_ordinal", &alias.proof),
        ("database_element_index", &database.original),
        ("database_declaration_ordinal", &database.original),
        ("event_database_entry_index", &database.resolution),
        ("timeline_element_index", &timeline.proof),
        ("parameter_entry_index", parameter_origin),
    ] {
        records::field(
            prepared,
            &format!("/data/payload/{field}"),
            proof.clone(),
            writer,
        )?;
    }
    Ok(())
}

fn origin<'record>(
    prepared: &'record PreparedRecordOrigins,
    path: &str,
) -> Result<&'record OriginContributions, MediaError> {
    prepared.visit(path).ok_or_else(|| {
        MediaError::Contract("animation event lookup lost original declaration evidence".into())
    })
}

fn borrowed() -> MediaError {
    MediaError::Contract("animation event binding observations already borrowed".into())
}

fn allocation(writer: &MediaWriter<'_>, cause: TryReserveError) -> MediaError {
    error::io(
        &writer.root,
        io::Error::new(io::ErrorKind::OutOfMemory, cause),
    )
}
