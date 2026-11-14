// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveModels, LoadoutChanges, LoadoutResolver, RigInput, RigInputs,
    discover_entity_clips_using,
};
use p4kx_extraction::{DataForge, DataForgeEntry, Entry};

use crate::lookup_work::LookupWork;
use crate::source_selection::EntrySelection;
use crate::{DirectoryLookup, MediaError, MediaWriter, Subresource, required_input};

#[path = "entity_catalogue_rows.rs"]
mod rows;

pub(super) use rows::bind_schema;

pub(super) fn emit(
    lookup: &DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if !lookup.chain.is_empty() {
        return Ok(());
    }
    let Some(selector) = writer.animation_entity.clone() else {
        return Ok(());
    };
    let Some(entry) = DataForgeEntry::locate(lookup.index.archive) else {
        return rows::unavailable(&selector, rows::CatalogueState::MissingDataforge, writer);
    };
    let selection = lookup.selection.classify(lookup.chain, entry.path());
    if selection != EntrySelection::Included {
        return rows::unavailable(
            &selector,
            rows::CatalogueState::DataforgeNotSelected,
            writer,
        );
    }
    let use_at = Subresource::new(rows::CATALOGUE, Vec::new());
    let input = required_input::with_retained_reference_input(
        lookup,
        entry,
        &use_at,
        writer,
        |input, writer| Ok((input.origin_input(writer)?, input.read_payload()?)),
    )?;
    let Some((origin, bytes)) = input else {
        return rows::unavailable(
            &selector,
            rows::CatalogueState::DataforgeUnavailable,
            writer,
        );
    };
    let Ok(forge) = DataForge::open(&bytes) else {
        return rows::unavailable(&selector, rows::CatalogueState::InvalidDataforge, writer);
    };
    project(&selector, &forge, &origin, lookup, writer)
}

fn project(
    selector: &str,
    forge: &DataForge<'_>,
    origin: &crate::origin_input::MediaOriginInput,
    lookup: &DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let work = Work { lookup, writer };
    work.charge(forge.records().len() as u64)?;
    let cancel = writer.checkpoint.cancel.clone();
    let stop = || cancel.is_cancelled();
    writer.checkpoint()?;
    let mut resolver = LoadoutResolver::new(forge).with_stop(&stop);
    writer.checkpoint()?;
    let owner = resolver
        .entities()
        .find(selector)
        .map_err(|error| MediaError::Contract(error.to_string()))?;
    let tree = resolver.resolve_with(&owner, &LoadoutChanges::new());
    writer.checkpoint()?;
    let tree = tree.map_err(|error| MediaError::Contract(error.to_string()))?;
    let state = if rows::incomplete_loadout(&tree, writer)? {
        rows::CatalogueState::IncompleteLoadout
    } else {
        rows::CatalogueState::Available
    };
    let mut inputs = Inputs { lookup, writer };
    let index = lookup.index.index(
        &Work {
            lookup,
            writer: inputs.writer,
        }
        .lookup_work(),
    )?;
    let models = ArchiveModels::from_path_index(index);
    let catalogue = discover_entity_clips_using(&models, &tree, &mut inputs)?;
    rows::write(selector, state, &owner, origin, lookup, &catalogue, writer)
}

struct Inputs<'a, 'dir, 'writer> {
    lookup: &'a DirectoryLookup<'dir>,
    writer: &'a mut MediaWriter<'writer>,
}

struct Work<'a, 'dir, 'writer> {
    lookup: &'a DirectoryLookup<'dir>,
    writer: &'a MediaWriter<'writer>,
}

impl Work<'_, '_, '_> {
    fn charge(&self, units: u64) -> Result<(), MediaError> {
        self.lookup_work().charge(units)
    }

    fn lookup_work(&self) -> LookupWork<'_> {
        let limits = self.writer.limits();
        LookupWork {
            limits,
            checkpoint: self.writer.checkpoint,
            current: self.lookup.work,
            source_allowance: self.lookup.allowance(limits.sources),
        }
    }
}

impl RigInputs for Inputs<'_, '_, '_> {
    type Error = MediaError;

    fn check(&mut self) -> Result<(), Self::Error> {
        Work {
            lookup: self.lookup,
            writer: self.writer,
        }
        .charge(1)
    }

    fn read(&mut self, entry: &Entry) -> Result<RigInput, Self::Error> {
        self.check()?;
        if self
            .lookup
            .selection
            .classify(self.lookup.chain, entry.path())
            != EntrySelection::Included
        {
            return Ok(RigInput::NotSelected);
        }
        let use_at = Subresource::new(rows::CATALOGUE, vec![entry.index() as u64]);
        let payload = required_input::with_retained_reference_input(
            self.lookup,
            entry,
            &use_at,
            self.writer,
            |input, _| input.read_payload(),
        )?;
        Ok(payload.map_or_else(
            || RigInput::Unreadable("selected source has no completed retention receipt".into()),
            RigInput::Read,
        ))
    }
}
