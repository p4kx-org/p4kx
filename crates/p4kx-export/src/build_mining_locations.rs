// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::slice;

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Record, TreeBuilder};

use super::original_input::OriginalCatalogInput;
use crate::archive_dataforge_query::{self, checkpoint};
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::{CatalogProjection, CatalogTextRegistry};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_v3::{
    authored_fields, mineable_composition_definition, mineable_element_definition,
};
use crate::contract_starmap::StarmapObject;
use crate::entity_selection::{self, Selected};
use crate::guid_text::CatalogGuid;
use crate::harvest_link::Links;
use crate::harvest_locations::{HarvestSurvey, Objects};
use crate::mining_locations::{
    HarvestableProviderPreset, MineableComposition, MineableElement, MiningLocationCoverage,
    MiningLocationOptions, MiningLocationSnapshot, MiningRecordSource, MiningStarmapObject,
};
use crate::record_observation::RecordObservation;
use crate::starmap::TypeNames;
use crate::starmap_containers::Placements;
use crate::texts::{TextSources, Texts};
use crate::{CatalogError, ExportAccountingError, Kind, harvest_preset, starmap};

#[path = "mining_location_rows.rs"]
mod rows;

#[path = "mining_location_occurrences.rs"]
mod occurrences;
use occurrences::HarvestLocationsRead;

const DEFINITION_KINDS: [Kind; 4] = [
    Kind::MineableElements,
    Kind::MineableCompositions,
    Kind::HarvestableProviderPresets,
    Kind::StarmapObjects,
];

/// Reads the local logical mining projection without building the complete Catalog.
///
/// # Errors
///
/// Returns an input, source-evidence or cancellation error. Recoverable unreadable
/// inputs and unresolved links are reported in the returned coverage.
pub fn read_mining_locations(
    archive: &Archive,
    options: MiningLocationOptions<'_>,
) -> Result<MiningLocationSnapshot, CatalogError> {
    archive_dataforge_query::read(archive, None, options.cancel, |entry| {
        read_archive(archive, entry, options)
    })
}

/// Reads the local mining projection using already decoded bytes from this Archive.
///
/// # Errors
///
/// Returns the same errors as [`read_mining_locations`], or
/// [`CatalogError::SourceOriginMismatch`] when the bytes came from another input.
pub fn read_mining_locations_from(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: MiningLocationOptions<'_>,
) -> Result<MiningLocationSnapshot, CatalogError> {
    archive_dataforge_query::read(archive, Some(entry), options.cancel, |entry| {
        read_archive(archive, entry, options)
    })
}

fn read_archive(
    archive: &Archive,
    entry: &DataForgeEntry,
    options: MiningLocationOptions<'_>,
) -> Result<MiningLocationSnapshot, CatalogError> {
    let cancel = options.cancel;
    let forge = entry.open().map_err(CatalogError::DataForge)?;
    let sources = TextSources::read_languages(archive, slice::from_ref(&options.language))?;
    let texts = Texts::parse(&sources)?;
    let origins = CatalogOrigins::new(archive, entry, &sources)?;
    let input = OriginalCatalogInput::new(&forge, &origins);
    let registry = CatalogTextRegistry::default();
    let projection = CatalogProjection::new(&texts, &input, &registry);
    let mut coverage = MiningLocationCoverage {
        language_available: sources.entry(options.language).is_some(),
        archive_inventory_incomplete: crate::export_archive_health::inventory(archive).is_some(),

        ..MiningLocationCoverage::default()
    };
    let DefinitionRows {
        mineable_elements,
        mineable_compositions,
        harvestable_provider_presets,
        starmap_records,
    } = DefinitionRows::read(&input, &projection, cancel, &mut coverage)?;
    let world = read_world(
        archive,
        &input,
        &projection,
        starmap_records,
        &harvestable_provider_presets,
        cancel,
        &mut coverage,
    )?;
    let game_texts = projection
        .finish_game_texts()?
        .into_iter()
        .map(Into::into)
        .collect();
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    Ok(MiningLocationSnapshot {
        build: archive.build().cloned(),
        language: options.language,
        mineable_elements,
        mineable_compositions,
        harvestable_provider_presets,
        harvestable_locations: world.locations.logical,
        harvestable_occurrences: world.locations.physical,
        starmap_objects: world.starmap_objects,
        game_texts,
        source_inventory,
        coverage,
    })
}

struct WorldRows {
    locations: HarvestLocationsRead,
    starmap_objects: Vec<MiningStarmapObject>,
}

fn read_world<'source>(
    archive: &Archive,
    input: &'source OriginalCatalogInput<'source, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    records: Vec<Record<'source>>,
    providers: &[HarvestableProviderPreset],
    cancel: Option<&CancelToken>,
    coverage: &mut MiningLocationCoverage,
) -> Result<WorldRows, CatalogError> {
    let capture = EntryCapture::for_input(input);
    let presets: HashSet<_> = providers
        .iter()
        .filter_map(|preset| preset.source.guid.value().copied())
        .collect();
    let survey = HarvestSurvey::new(&presets);
    let placements = Placements::read_for_harvest(archive, cancel, &capture, &survey)?;
    coverage.placement_inventory_incomplete = !placements.ledger.incomplete_inventories.is_empty();
    coverage.unavailable_placement_occurrences =
        placements.counts.unreadable + placements.counts.missing + placements.counts.malformed;
    let objects = place_starmap(input, projection, records, &placements, cancel, coverage)?;
    let locations = harvest_locations(
        archive, cancel, &capture, placements, survey, &objects, coverage,
    )?;
    let starmap_objects = objects
        .into_iter()
        .map(|(source, object)| {
            checkpoint(cancel)?;
            rows::starmap(object, source, input.origins())
        })
        .collect::<Result<_, _>>()?;
    Ok(WorldRows {
        locations,
        starmap_objects,
    })
}
#[derive(Default)]
struct DefinitionRows<'source> {
    mineable_elements: Vec<MineableElement>,
    mineable_compositions: Vec<MineableComposition>,
    harvestable_provider_presets: Vec<HarvestableProviderPreset>,
    starmap_records: Vec<Record<'source>>,
}

impl<'source> DefinitionRows<'source> {
    fn read(
        input: &'source OriginalCatalogInput<'source, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        cancel: Option<&CancelToken>,
        coverage: &mut MiningLocationCoverage,
    ) -> Result<Self, CatalogError> {
        let mut out = Self::default();
        let mut trees = TreeBuilder::new();
        let links = Links::default();
        for record in input.forge().records() {
            checkpoint(cancel)?;
            let kind = match selected_kind(&record) {
                Ok(Some(kind)) => kind,
                Ok(None) => continue,
                Err(_) => {
                    coverage.unreadable_record_headers += 1;
                    continue;
                }
            };
            if kind == Kind::StarmapObjects {
                out.starmap_records.push(record);
                continue;
            }
            let Ok(tree) = trees.build(&record) else {
                coverage.unreadable_record_bodies += 1;
                continue;
            };
            if kind == Kind::MineableElements {
                let admission = RecordObservation::capture(input, &record)?;
                let (row, notes) =
                    mineable_element_definition::read(&tree, projection, &admission, cancel)?;
                coverage.unreadable_mining_fields += notes.len();
                out.mineable_elements.push(row);
                continue;
            }
            match kind {
                Kind::MineableCompositions => {
                    let admission = RecordObservation::capture(input, &record)?;
                    let read = mineable_composition_definition::read(
                        &tree,
                        &projection.for_kind(kind),
                        &admission,
                        cancel,
                    )?;
                    coverage.unreadable_text_fields += read.unreadable_text_fields;
                    coverage.unreadable_mining_fields +=
                        read.notes.len() - read.unreadable_text_fields;
                    out.mineable_compositions.push(read.value);
                }
                Kind::HarvestableProviderPresets => {
                    let source = MiningRecordSource::from(CanonicalRecordHeader::read(
                        &tree,
                        input.origins(),
                    )?);
                    let row =
                        harvest_preset::provider_preset(&mut trees, &tree, input.forge(), &links);
                    out.harvestable_provider_presets.push((row, source).into());
                }
                _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
            }
        }
        coverage.unreadable_provider_links = links.unreadable_count();
        out.order();
        Ok(out)
    }

    fn order(&mut self) {
        MineableElement::order(&mut self.mineable_elements);
        MineableComposition::order(&mut self.mineable_compositions);
        self.harvestable_provider_presets
            .sort_by_cached_key(|row| source_guid(&row.source));
    }
}

fn source_guid(source: &MiningRecordSource) -> Option<crate::guid_text::GuidText> {
    source.guid.value().map(|guid| CatalogGuid(*guid).text())
}

fn selected_kind(record: &Record<'_>) -> Result<Option<Kind>, p4kx_extraction::DataForgeError> {
    let path = record.record_path()?;
    if !DEFINITION_KINDS
        .into_iter()
        .any(|kind| kind.records().matches_path(path))
    {
        return Ok(None);
    }
    Ok(match entity_selection::select(record, None)? {
        Some(Selected::Entity(kind)) if DEFINITION_KINDS.contains(&kind) => Some(kind),
        _ => None,
    })
}

fn place_starmap<'source>(
    input: &'source OriginalCatalogInput<'source, 'source>,
    projection: &CatalogProjection<'_, 'source>,
    records: Vec<Record<'source>>,
    placements: &Placements,
    cancel: Option<&CancelToken>,
    coverage: &mut MiningLocationCoverage,
) -> Result<Vec<(MiningRecordSource, StarmapObject<'source>)>, CatalogError> {
    let forge = input.forge();
    let mut trees = TreeBuilder::new();
    let types = TypeNames::default();
    let projection = projection.for_kind(Kind::StarmapObjects);
    let mut objects = Vec::with_capacity(records.len());
    for record in records {
        checkpoint(cancel)?;
        let Ok(tree) = trees.build(&record) else {
            coverage.unreadable_record_bodies += 1;
            continue;
        };
        let source = CanonicalRecordHeader::read(&tree, input.origins())?.into();
        let admission = RecordObservation::capture(input, &record)?;
        let (name, notes) = authored_fields::read_root_locale_or_legacy_key(
            &tree,
            &projection,
            &admission,
            cancel,
            "name",
        )?;
        coverage.unreadable_text_fields += notes.len();
        let object = starmap::starmap_object(&mut trees, &tree, name, forge, &types, placements);
        objects.push((source, object));
    }
    objects.sort_by_cached_key(|(_, row)| (row.guid.text(), row.record_index));
    Ok(objects)
}

fn harvest_locations(
    archive: &Archive,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
    mut placements: Placements,
    survey: HarvestSurvey<'_>,
    starmap_objects: &[(MiningRecordSource, StarmapObject<'_>)],
    coverage: &mut MiningLocationCoverage,
) -> Result<HarvestLocationsRead, CatalogError> {
    let opened = std::mem::take(&mut placements.opened);
    let routed = std::mem::take(&mut placements.harvest_occurrences);
    drop(placements);
    let objects = Objects::new(starmap_objects.iter().map(|(_, row)| row));
    let (located, positives) = survey.finish(archive, &opened, &objects, cancel, capture)?;
    coverage.unreadable_harvest_containers = located.unreadable.len();
    coverage.locations_with_unresolved_parent =
        located.without_object.iter().map(|(_, count)| count).sum();
    let physical = occurrences::project(
        archive, capture, cancel, &routed, &opened, &positives, &objects,
    )?;
    Ok(HarvestLocationsRead {
        logical: located.locations.into_iter().map(Into::into).collect(),
        physical,
    })
}

#[cfg(test)]
#[path = "mining_location_reader_tests.rs"]
mod tests;
