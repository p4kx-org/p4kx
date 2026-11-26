// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, PhysicalSourceRef, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::blueprint_recipe_coordinates::{
    BlueprintKey, CostPosition, PATH as TIERS, RecipeCoordinates,
};
use super::blueprint_tiers;
use super::cell::Cell;
use super::completeness;
use super::dismantle_processes::DismantleProcesses;
use super::record_identity::RecordIdentity;
use super::record_targets::RecordTargets;
use super::selected_texts::{SelectedTexts, TextRefs};
use crate::CatalogError;
use crate::blueprint_definition::{BlueprintDefinition, PhysicalBlueprintRef};
use crate::blueprint_presentation::BlueprintPresentation;
use crate::blueprint_tier_contract::BlueprintTier;
use crate::catalog_queries::{BlueprintSnapshot, CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;

pub(super) const DEFINITIONS: &str = "definitions.blueprints";
pub(super) const PRESENTATIONS: &str = "projections.blueprintPresentation";

pub(super) struct Blueprints {
    pub(super) definitions: StagedCatalogTable,
    pub(super) presentations: StagedCatalogTable,
    pub(super) tiers: StagedCatalogTable,
    pub(super) targets: RecordTargets,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
    processes: &DismantleProcesses,
) -> Result<Blueprints, ReducedError> {
    parsed.with_blueprints(CatalogRecordSelection::All, cancel, |snapshot| {
        stage(&snapshot, scratch, cancel, texts, processes)
    })?
}

fn stage(
    snapshot: &BlueprintSnapshot<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
    processes: &DismantleProcesses,
) -> Result<Blueprints, ReducedError> {
    let availability = snapshot.availability();
    completeness::inventory(DEFINITIONS, availability, snapshot.coverage())?;
    if !snapshot.unsupported_ranges().is_empty() {
        return Err(ReducedError::IncompleteScope(TIERS));
    }
    completeness::rows(
        PRESENTATIONS,
        snapshot.definitions().len(),
        snapshot.presentations().len(),
    )?;
    let mut targets = RecordTargets::default();
    let mut rows = Rows::new(scratch, cancel);
    let mut references = TextRefs::default();
    let mut remaining_tiers = snapshot.tiers();
    for (definition, presentation) in snapshot.definitions().iter().zip(snapshot.presentations()) {
        let header = definition.header();
        targets.insert(DEFINITIONS, header)?;
        let owner = Owner::read(definition)?;
        if !owner.matches(presentation.blueprint()) {
            return Err(ReducedError::IncompleteScope(PRESENTATIONS));
        }
        let tier_count = remaining_tiers
            .iter()
            .take_while(|tier| owner.matches(tier.blueprint()))
            .count();
        let (owner_tiers, rest) = remaining_tiers.split_at(tier_count);
        remaining_tiers = rest;
        rows.append(
            definition,
            owner.key,
            presentation,
            owner_tiers,
            &mut references,
            processes,
        )?;
    }
    completeness::rows(PRESENTATIONS, 0, remaining_tiers.len())?;
    rows.finish(snapshot, targets, texts, references)
}

struct Rows<'stage> {
    definitions: StagedCatalogRows<'stage>,
    presentations: StagedCatalogRows<'stage>,
    tiers: StagedCatalogRows<'stage>,
    cancel: Option<&'stage CancelToken>,
}

impl<'stage> Rows<'stage> {
    const fn new(scratch: &'stage Path, cancel: Option<&'stage CancelToken>) -> Self {
        Self {
            definitions: StagedCatalogRows::new(scratch, cancel),
            presentations: StagedCatalogRows::new(scratch, cancel),
            tiers: StagedCatalogRows::new(scratch, cancel),
            cancel,
        }
    }

    fn append<'read>(
        &mut self,
        definition: &'read BlueprintDefinition<'_>,
        owner: BlueprintKey,
        presentation: &'read BlueprintPresentation<SourcedValue<CatalogGuid>>,
        tiers: &[BlueprintTier],
        references: &mut TextRefs<'read>,
        processes: &DismantleProcesses,
    ) -> Result<(), ReducedError> {
        let coordinates = RecipeCoordinates::new(tiers, self.cancel)?;
        self.definitions
            .append(&RecordIdentity::from(definition.header()))?;
        self.presentations.append(&Presentation::read(
            owner,
            presentation,
            &coordinates,
            references,
            processes,
            self.cancel,
        )?)?;
        blueprint_tiers::append(
            &mut self.tiers,
            owner,
            tiers,
            &coordinates,
            definition.source(),
            self.cancel,
        )
    }

    fn finish(
        self,
        snapshot: &BlueprintSnapshot<'_>,
        targets: RecordTargets,
        texts: &mut SelectedTexts,
        references: TextRefs<'_>,
    ) -> Result<Blueprints, ReducedError> {
        let availability = snapshot.availability();
        let result = Blueprints {
            definitions: self.definitions.finish(availability)?,
            presentations: self.presentations.finish(availability)?,
            tiers: self.tiers.finish(availability)?,
            targets,
        };
        completeness::rows(
            DEFINITIONS,
            snapshot.definitions().len(),
            result.definitions.len(),
        )?;
        completeness::rows(
            PRESENTATIONS,
            result.definitions.len(),
            result.presentations.len(),
        )?;
        completeness::rows(TIERS, snapshot.tiers().len(), result.tiers.len())?;
        texts.add(
            references,
            snapshot.public_names(),
            snapshot.game_texts(),
            self.cancel,
        )?;
        Ok(result)
    }
}

struct Owner<'read> {
    input_ref: &'read PhysicalSourceRef,
    key: BlueprintKey,
}

impl<'read> Owner<'read> {
    fn read(definition: &'read BlueprintDefinition<'_>) -> Result<Self, ReducedError> {
        let header = definition.header();
        completeness::value(DEFINITIONS, header.class_name())?;
        completeness::value(DEFINITIONS, header.record_path())?;
        Ok(Self {
            input_ref: definition.input_ref(),
            key: BlueprintKey {
                record_index: *header
                    .record_index()
                    .value()
                    .ok_or(ReducedError::IncompleteScope(DEFINITIONS))?,
            },
        })
    }

    fn matches(&self, source: &PhysicalBlueprintRef) -> bool {
        source.input_ref() == self.input_ref && source.record_index() == self.key.record_index
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Presentation<'read> {
    blueprint: BlueprintKey,
    name: Cell<&'read GameTextRef>,
    category: Cell<&'read CatalogGuid>,
    produced_item: Cell<&'read CatalogGuid>,
    first_tier: Option<u32>,
    ingredients: Vec<Ingredient>,
    default_available: Cell<&'read bool>,
    dismantle: Cell<Dismantle>,
}

impl<'read> Presentation<'read> {
    fn read(
        blueprint: BlueprintKey,
        source: &'read BlueprintPresentation<SourcedValue<CatalogGuid>>,
        coordinates: &RecipeCoordinates<'_>,
        references: &mut TextRefs<'read>,
        processes: &DismantleProcesses,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, ReducedError> {
        let name = completeness::cell(PRESENTATIONS, source.name())?;
        references.game_text(name.value);
        let first_tier = source
            .first_tier()
            .map(|tier| coordinates.tier(tier))
            .transpose()?;
        let mut ingredients = Vec::new();
        ingredients
            .try_reserve_exact(source.ingredients().len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for ingredient in source.ingredients() {
            let tier = first_tier.ok_or(ReducedError::IncompleteScope(PRESENTATIONS))?;
            let (cost, slot) =
                coordinates.ingredient(ingredient.cost(), ingredient.slot(), tier, cancel)?;
            ingredients.push(Ingredient { cost, slot });
        }
        Ok(Self {
            blueprint,
            name,
            category: completeness::cell(PRESENTATIONS, source.category())?,
            produced_item: completeness::cell(PRESENTATIONS, source.produced_item())?,
            first_tier,
            ingredients,
            default_available: completeness::cell(PRESENTATIONS, source.default_available())?,
            dismantle: completeness::cell(PRESENTATIONS, source.dismantle())?.try_map(|value| {
                Ok::<_, ReducedError>(Dismantle {
                    process_guid: value.process_guid(),
                    process_record_index: processes.unique_record(value.process_guid())?,
                    tier: value
                        .tier()
                        .map(|tier| coordinates.tier(tier))
                        .transpose()?,
                })
            })?,
        })
    }
}

#[derive(Serialize)]
struct Ingredient {
    cost: CostPosition,
    slot: Option<CostPosition>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Dismantle {
    process_guid: CatalogGuid,
    process_record_index: u32,
    tier: Option<u32>,
}
