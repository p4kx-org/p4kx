// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, TreeBuilder};

use super::{LootDefinitionKind, LootDefinitionSnapshot};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::unlocalized_record_query::plan::{Owner, Plan};
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::loot_definition_catalog;
use crate::guid_text::sort_by_guid;
use crate::harvest_config::SlotDependencies;
use crate::harvest_link::{DependencyReader, Links};
use crate::{CatalogError, CraftingWarning};

pub(super) struct Query<'query> {
    pub(super) archive: &'query Archive,
    pub(super) entry: CatalogQuerySource<'query>,
    pub(super) forge: &'query DataForge<'query>,
    pub(super) family: LootDefinitionKind,
    pub(super) selection: CatalogRecordSelection,
    pub(super) cancel: Option<&'query CancelToken>,
}

impl<'source> Query<'source> {
    pub(super) fn run<T>(
        self,
        inspect: impl FnOnce(LootDefinitionSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        let mut plan = Plan::read(
            self.forge,
            self.family.kind(),
            self.selection,
            self.cancel,
            None,
        )?;
        let origins = self.entry.records(self.archive)?;
        let input = OriginalCatalogInput::new(self.forge, &origins);
        let rows = self.read(plan.owners, &input, &mut plan.coverage)?;
        let table = CatalogTable::new(
            self.forge,
            &[self.family.rule().type_name],
            rows.definitions,
        );
        let source_inventory = origins.uses.finish_with(|| checkpoint(self.cancel))?;
        checkpoint(self.cancel)?;
        Ok(inspect(LootDefinitionSnapshot {
            build: self.archive.build().cloned(),
            availability: table.availability,
            rows: table.rows,
            tables: rows.tables,
            archetypes: rows.archetypes,
            sub_harvestable_multi_configs: rows.sub_harvestable_multi_configs,
            harvest_dependency_warning: rows.harvest_dependency_warning,
            source_inventory,
            coverage: plan.coverage,
        }))
    }

    fn read(
        &self,
        owners: Vec<Owner<'source>>,
        input: &OriginalCatalogInput<'_, 'source>,
        coverage: &mut CatalogRecordCoverage,
    ) -> Result<Rows<'source>, CatalogError> {
        let mut rows = Rows::default();
        let mut harvest_trees = TreeBuilder::new();
        let harvest_links = Links::default();
        for owner in owners {
            checkpoint(self.cancel)?;
            let definition = match self.family {
                LootDefinitionKind::Table => {
                    let (definition, table) = loot_definition_catalog::read_table(
                        &owner.record,
                        &owner.tree,
                        input,
                        self.cancel,
                    )?;
                    rows.tables.push(table);
                    definition
                }
                LootDefinitionKind::Archetype => {
                    let (definition, archetype) = loot_definition_catalog::read_archetype(
                        &owner.record,
                        &owner.tree,
                        input,
                        self.cancel,
                    )?;
                    rows.archetypes.push(archetype);
                    definition
                }
                LootDefinitionKind::SubHarvestableMultiConfig => {
                    let (definition, projection) = loot_definition_catalog::read_multi_config(
                        &owner.record,
                        &owner.tree,
                        &mut SlotDependencies {
                            reader: DependencyReader {
                                trees: &mut harvest_trees,
                                forge: self.forge,
                                input,
                                cancel: self.cancel,
                            },
                            links: &harvest_links,
                        },
                    )?;
                    rows.sub_harvestable_multi_configs.extend(projection);
                    definition
                }
                _ => loot_definition_catalog::read(&owner.record, &owner.tree, input, self.cancel)?,
            };
            if definition.incomplete_fields {
                coverage.incomplete_fields.push(owner.record.index());
            }
            rows.definitions.push(definition);
        }
        sort_by_guid(&mut rows.definitions, |row| row.physical_guid);
        sort_by_guid(&mut rows.tables, |row| row.guid);
        sort_by_guid(&mut rows.archetypes, |row| row.guid);
        sort_by_guid(&mut rows.sub_harvestable_multi_configs, |row| row.guid);
        rows.harvest_dependency_warning = harvest_links.warning();
        Ok(rows)
    }
}

#[derive(Default)]
struct Rows<'source> {
    definitions: Vec<super::LootDefinition<'source>>,
    tables: Vec<super::LootTable<'source>>,
    archetypes: Vec<super::LootArchetype<'source>>,
    sub_harvestable_multi_configs: Vec<super::SubHarvestableMultiConfig<'source>>,
    harvest_dependency_warning: Option<CraftingWarning>,
}
