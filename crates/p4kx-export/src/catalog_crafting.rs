// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;

use p4kx_extraction::{DataForge, Filter};

use crate::CatalogError;
use crate::CatalogOptions;
use crate::Kind;
use crate::blueprint_catalog_rows::{self, BlueprintRow};
use crate::blueprint_definition::BlueprintDefinition;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read_failure::CatalogReadFailure;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_v3::crafting_global::PreparedCraftingGlobal;
use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyDefinition;
use crate::catalog_v3::industry_modules::blueprint_pool_definition::PreparedBlueprintPool;
use crate::catalog_v3::industry_modules::dismantle_process_definition::PreparedDismantleProcess;
use crate::crafting::{self, Crafting, ReadCrafting};
use crate::crafting_types::BlueprintPool;
use crate::crafting_warning::CraftingWarning;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::quality_catalog::QualityRows;

#[path = "crafting_ledger/catalog_rows.rs"]
mod rows;

/// Selected crafting and canonical source datasets in their documented GUID order.
pub(crate) struct CraftingKinds<'f> {
    pub(crate) blueprints: Vec<BlueprintRow<'f>>,
    pub(crate) properties: Vec<GameplayPropertyDefinition<'f>>,
    pub(crate) quality: QualityRows<'f>,
    pub(crate) pools: Vec<PreparedBlueprintPool<'f>>,
    pub(crate) reward_declarations: Vec<
        crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource<'f>,
    >,
    pub(crate) reward_numbers: Vec<crate::catalog_number_capture::CapturedNumber>,
    pub(crate) dismantle: Vec<PreparedDismantleProcess<'f>>,
    pub(crate) global: Vec<PreparedCraftingGlobal<'f>>,
    pub(crate) skipped: Vec<(Kind, usize)>,
    pub(crate) failures: Vec<CatalogReadFailure<'f>>,
    pub(crate) warnings: Vec<CraftingWarning>,
    pub(crate) ledger: ExportLedger,
}

/// Reads and projects the catalog's selected crafting datasets.
pub(crate) fn read<'source>(
    forge: &'source DataForge<'_>,
    options: &CatalogOptions<'_>,
    projection: &CatalogProjection<'_, 'source>,
) -> Result<(CraftingKinds<'source>, StagedCatalogTable), CatalogError> {
    let input = crafting::read_with_progress(
        forge,
        options.filter,
        options.cancel,
        options.progress,
        projection,
    )?;
    let directory = std::env::temp_dir();
    let mut rows = StagedCatalogRows::new(&directory, options.cancel);
    let kinds = kinds(input, projection.origins(), options.filter, |definition| {
        rows.append(definition).map_err(CatalogError::from)
    })?;
    let availability = blueprint_catalog_rows::availability(forge, kinds.blueprints.len());
    Ok((kinds, rows.finish(availability)?))
}

/// Combines canonical gameplay and quality facts with interpreted crafting datasets.
/// Other crafting rows apply their own catalog Filter and development-leftover rules.
pub(crate) fn kinds<'f>(
    read: ReadCrafting<'f>,
    origins: &CatalogOrigins,
    filter: Option<&Filter>,
    emit_blueprint: impl FnMut(&BlueprintDefinition<'f>) -> Result<(), CatalogError>,
) -> Result<CraftingKinds<'f>, CatalogError> {
    let ReadCrafting {
        data: crafting,
        blueprints,
        blueprint_rules,
        globals,
        dismantle,
        pools: mut original_pools,
        mut reward_declarations,
        mut reward_numbers,
        catalog_failures,
        mut ledger,
        gameplay,
        quality,
    } = read;
    let mut selected = rows::Selection::new(filter, &mut ledger);
    let properties = gameplay.definitions;
    let properties_skipped = gameplay.skipped;
    let distributions_skipped = quality.skipped_distributions;
    let quantizations_skipped = quality.skipped_quantizations;
    let pools_skipped = select_pools(&crafting.pools, &mut original_pools, &mut selected);
    retain_rewards(
        &mut reward_declarations,
        &mut reward_numbers,
        &original_pools,
    )?;
    let (kept, dismantle_skipped) = dismantle_guids(&crafting, &mut selected);
    let process = crafting
        .dismantle_process()
        .map(|item| item.guid)
        .filter(|guid| kept.contains(&CatalogGuid(*guid)));
    let blueprints = blueprints
        .into_iter()
        .map(|row| blueprint_catalog_rows::row(row, &blueprint_rules, process, origins))
        .collect::<Result<Vec<_>, _>>()?;
    let blueprints = blueprint_catalog_rows::retain_definitions(blueprints, emit_blueprint)?;
    Ok(CraftingKinds {
        blueprints,
        properties,
        quality,
        pools: original_pools,
        reward_declarations,
        reward_numbers,
        dismantle,
        global: globals,
        skipped: vec![
            (Kind::Blueprints, crafting.skipped_blueprints),
            (Kind::GameplayProperties, properties_skipped),
            (Kind::QualityDistributions, distributions_skipped),
            (Kind::QualityQuantizations, quantizations_skipped),
            (Kind::BlueprintPools, pools_skipped),
            (Kind::DismantleProcesses, dismantle_skipped),
        ],
        failures: catalog_failures,
        warnings: crafting.warnings,
        ledger,
    })
}

fn select_pools(
    pools: &[BlueprintPool<'_>],
    originals: &mut Vec<PreparedBlueprintPool<'_>>,
    selection: &mut rows::Selection<'_>,
) -> usize {
    let (kept, skipped) = selection.entities(
        Kind::BlueprintPools,
        pools,
        |item| (item.record_path, item.class_name, CatalogGuid(item.guid)),
        |source| (CatalogGuid(source.guid), source.record_path),
        |entry| entry.0,
    );
    let kept: HashSet<_> = kept
        .into_iter()
        .map(|(guid, path)| (guid.0, path))
        .collect();
    originals.retain(|row| kept.contains(&(row.physical_guid, row.physical_path)));
    skipped
}

fn dismantle_guids(
    crafting: &Crafting<'_>,
    selected: &mut rows::Selection<'_>,
) -> (Vec<CatalogGuid>, usize) {
    selected.entities(
        Kind::DismantleProcesses,
        &crafting.dismantle,
        |item| (item.record_path, item.class_name, CatalogGuid(item.guid)),
        |source| CatalogGuid(source.guid),
        |guid| *guid,
    )
}

fn retain_rewards(
    declarations: &mut Vec<
        crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource<'_>,
    >,
    numbers: &mut Vec<crate::catalog_number_capture::CapturedNumber>,
    pools: &[PreparedBlueprintPool<'_>],
) -> Result<(), CatalogError> {
    let wanted: HashSet<_> = pools.iter().map(|row| row.physical_guid).collect();
    declarations.retain(|row| {
        row.pool().value().is_some_and(|pool| {
            pool.guid()
                .value()
                .is_some_and(|guid| wanted.contains(guid))
        })
    });
    let mut positions = HashSet::new();
    for row in declarations {
        positions.extend(row.number_position()?);
    }
    numbers.retain(|number| match number.occurrence {
        crate::catalog_number_capture::NumberOccurrence::ConfigurationRawValue { ordinal } => {
            positions.contains(&(number.record_index, ordinal))
        }
        _ => false,
    });
    Ok(())
}
