// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Record, RecordTree, Value};

use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::NumberCapture;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueSite};
use crate::contract_loot::{LootArchetype, LootTable};
use crate::export_field_error;
use crate::guid_text::CatalogGuid;
use crate::loot_records;

use super::definition::LootDefinition;
use crate::source_value_catalog::{self, SourceArenaOutput};

/// Reads every original field and value once without following Reference target bodies.
pub(crate) fn read<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<LootDefinition<'source>, CatalogError> {
    let scope = input.record_scope(record, tree)?;
    let mut observation = FieldObservation::default();
    let output = source_value_catalog::read_arena(&scope, cancel, |site| observation.read(site))?;
    definition(tree, input, output, observation)
}

/// Reads the original table and its weighted view from the same retained occurrences.
pub(crate) fn read_table<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(LootDefinition<'source>, LootTable<'source>), CatalogError> {
    read_projected(record, tree, input, cancel, |facts| {
        loot_records::loot_table(tree, facts)
    })
}

/// Reads the original archetype and its weighted view from the same retained occurrences.
pub(crate) fn read_archetype<'source>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(LootDefinition<'source>, LootArchetype<'source>), CatalogError> {
    read_projected(record, tree, input, cancel, |facts| {
        loot_records::loot_archetype(tree, facts)
    })
}

pub(crate) fn read_projected<'source, T>(
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    cancel: Option<&CancelToken>,
    project: impl FnOnce(&OriginalFlowFields<'_, '_, 'source>) -> Result<T, CatalogError>,
) -> Result<(LootDefinition<'source>, T), CatalogError> {
    let scope = input.record_scope(record, tree)?;
    let mut observation = FieldObservation::default();
    let indexed =
        source_value_catalog::read_indexed_arena(&scope, cancel, |site| observation.read(site))?;
    let projected = project(&indexed.facts)?;
    Ok((
        definition(tree, input, indexed.source, observation)?,
        projected,
    ))
}

fn definition<'source>(
    tree: &RecordTree<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    output: SourceArenaOutput<'source>,
    observation: FieldObservation,
) -> Result<LootDefinition<'source>, CatalogError> {
    Ok(LootDefinition {
        physical_guid: CatalogGuid(tree.guid()),
        physical_path: tree.record_path(),
        numbers: observation.numbers.finish(),
        incomplete_fields: observation.incomplete,
        source: CanonicalRecordHeader::read(tree, input.origins())?,
        fields: output.fields,
        values: output.values,
    })
}

#[derive(Default)]
struct FieldObservation {
    numbers: NumberCapture,
    incomplete: bool,
}

impl FieldObservation {
    fn read(&mut self, site: &OriginalValueSite<'_, '_, '_, '_>) {
        self.numbers.retain_loot(site);
        self.incomplete |= matches!(site.value(), Value::Error(error) if export_field_error::blocks_projection(*error));
    }
}
