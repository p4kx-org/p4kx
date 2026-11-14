// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, Guid};
use schemars::JsonSchema;
use serde::Serialize;

use super::{LootDefinition, LootDefinitionKind};
use crate::Kind;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_table::CatalogTable;
use crate::guid_text::sort_by_guid;
use crate::schema_fields::every_field_required;

/// Canonical physical Loot Records by exact source family.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LootDefinitionTables<'source> {
    #[serde(rename = "lootPoolFilters")]
    pool_filters: CatalogTable<LootDefinition<'source>>,
    #[serde(rename = "lootSecondaryChoiceSingleLayers")]
    secondary_choice_single_layers: CatalogTable<LootDefinition<'source>>,
    #[serde(rename = "lootSecondaryChoiceMultiLayers")]
    secondary_choice_multi_layers: CatalogTable<LootDefinition<'source>>,
    #[serde(rename = "lootThemes")]
    themes: CatalogTable<LootDefinition<'source>>,
    hierarchical_contextual_data_presets: CatalogTable<LootDefinition<'source>>,
    loot_generation_global_params: CatalogTable<LootDefinition<'source>>,
    sub_harvestable_configs: CatalogTable<LootDefinition<'source>>,
    sub_harvestable_multi_configs: CatalogTable<LootDefinition<'source>>,
}

/// Collected source-owned Loot definitions.
#[derive(Default)]
pub(crate) struct LootDefinitions<'source> {
    pool_filters: Vec<LootDefinition<'source>>,
    secondary_single: Vec<LootDefinition<'source>>,
    secondary_multi: Vec<LootDefinition<'source>>,
    themes: Vec<LootDefinition<'source>>,
    hierarchical_contextual_data_presets: Vec<LootDefinition<'source>>,
    global_params: Vec<LootDefinition<'source>>,
    sub_harvestable_configs: Vec<LootDefinition<'source>>,
    sub_harvestable_multi_configs: Vec<LootDefinition<'source>>,
}

impl<'source> LootDefinitions<'source> {
    pub(crate) fn push(&mut self, kind: LootDefinitionKind, row: LootDefinition<'source>) {
        match kind {
            LootDefinitionKind::PoolFilter => &mut self.pool_filters,
            LootDefinitionKind::SecondaryChoiceSingleLayer => &mut self.secondary_single,
            LootDefinitionKind::SecondaryChoiceMultiLayer => &mut self.secondary_multi,
            LootDefinitionKind::Theme => &mut self.themes,
            LootDefinitionKind::HierarchicalContextualDataPreset => {
                &mut self.hierarchical_contextual_data_presets
            }
            LootDefinitionKind::GlobalParams => &mut self.global_params,
            LootDefinitionKind::SubHarvestableConfig => &mut self.sub_harvestable_configs,
            LootDefinitionKind::SubHarvestableMultiConfig => {
                &mut self.sub_harvestable_multi_configs
            }
        }
        .push(row);
    }

    pub(crate) fn order(&mut self) {
        for rows in [
            &mut self.pool_filters,
            &mut self.secondary_single,
            &mut self.secondary_multi,
            &mut self.themes,
            &mut self.hierarchical_contextual_data_presets,
            &mut self.global_params,
            &mut self.sub_harvestable_configs,
            &mut self.sub_harvestable_multi_configs,
        ] {
            sort_by_guid(rows, |row| row.physical_guid);
        }
    }

    pub(crate) const fn count(&self, kind: LootDefinitionKind) -> usize {
        self.rows(kind).len()
    }

    pub(crate) fn sources(
        &self,
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &[CapturedNumber])> {
        LootDefinitionKind::ALL.into_iter().flat_map(|kind| {
            self.rows(kind).iter().map(move |row| {
                (
                    kind.kind(),
                    row.physical_guid.0,
                    row.physical_path,
                    row.numbers.as_slice(),
                )
            })
        })
    }

    pub(crate) fn tables(self, forge: &DataForge<'_>) -> LootDefinitionTables<'source> {
        use LootDefinitionKind::{
            GlobalParams, HierarchicalContextualDataPreset, PoolFilter, SecondaryChoiceMultiLayer,
            SecondaryChoiceSingleLayer, SubHarvestableConfig, SubHarvestableMultiConfig, Theme,
        };

        let table = |kind: LootDefinitionKind, rows| {
            CatalogTable::new(forge, &[kind.rule().type_name], rows)
        };
        LootDefinitionTables {
            pool_filters: table(PoolFilter, self.pool_filters),
            secondary_choice_single_layers: table(
                SecondaryChoiceSingleLayer,
                self.secondary_single,
            ),
            secondary_choice_multi_layers: table(SecondaryChoiceMultiLayer, self.secondary_multi),
            themes: table(Theme, self.themes),
            hierarchical_contextual_data_presets: table(
                HierarchicalContextualDataPreset,
                self.hierarchical_contextual_data_presets,
            ),
            loot_generation_global_params: table(GlobalParams, self.global_params),
            sub_harvestable_configs: table(SubHarvestableConfig, self.sub_harvestable_configs),
            sub_harvestable_multi_configs: table(
                SubHarvestableMultiConfig,
                self.sub_harvestable_multi_configs,
            ),
        }
    }

    const fn rows(&self, kind: LootDefinitionKind) -> &[LootDefinition<'source>] {
        match kind {
            LootDefinitionKind::PoolFilter => self.pool_filters.as_slice(),
            LootDefinitionKind::SecondaryChoiceSingleLayer => self.secondary_single.as_slice(),
            LootDefinitionKind::SecondaryChoiceMultiLayer => self.secondary_multi.as_slice(),
            LootDefinitionKind::Theme => self.themes.as_slice(),
            LootDefinitionKind::HierarchicalContextualDataPreset => {
                self.hierarchical_contextual_data_presets.as_slice()
            }
            LootDefinitionKind::GlobalParams => self.global_params.as_slice(),
            LootDefinitionKind::SubHarvestableConfig => self.sub_harvestable_configs.as_slice(),
            LootDefinitionKind::SubHarvestableMultiConfig => {
                self.sub_harvestable_multi_configs.as_slice()
            }
        }
    }
}
