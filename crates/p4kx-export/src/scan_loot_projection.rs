// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, TreeBuilder};

use super::SelectedProjection;
use crate::catalog_v3::loot_definition_catalog::{self, LootDefinitionKind};
use crate::harvest_config::SlotDependencies;
use crate::harvest_link::DependencyReader;
use crate::loot_kinds::LootPick;
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

impl<'source> SelectedProjection<'_, '_, '_, 'source> {
    pub(super) fn loot_definition(
        &self,
        kind: Kind,
        trees: &mut TreeBuilder<'source>,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
    ) -> Result<Option<Picked<'source>>, CatalogError> {
        if let Some(legacy) = LootPick::read_legacy(
            kind,
            admission.record(),
            tree,
            &self.projection,
            self.scan.options.cancel,
        )? {
            return Ok(Some(Picked::Loot(legacy)));
        }
        if let Some(loot) = LootDefinitionKind::of(kind) {
            if let Some(weighted) = LootPick::read_weighted(
                loot,
                admission.record(),
                tree,
                &self.projection,
                self.scan.options.cancel,
            )? {
                return Ok(Some(Picked::Loot(weighted)));
            }
            if loot == LootDefinitionKind::SubHarvestableMultiConfig {
                let (definition, projection) = loot_definition_catalog::read_multi_config(
                    admission.record(),
                    tree,
                    &mut SlotDependencies {
                        reader: DependencyReader {
                            trees,
                            forge: self.scan.forge,
                            input: self.projection.original(),
                            cancel: self.scan.options.cancel,
                        },
                        links: &self.scan.links,
                    },
                )?;
                return Ok(Some(Picked::MultiConfig {
                    definition: Box::new(definition),
                    projection,
                }));
            }
            let definition = loot_definition_catalog::read(
                admission.record(),
                tree,
                self.projection.original(),
                self.scan.options.cancel,
            )?;
            return Ok(Some(Picked::Loot(LootPick::Definition(
                loot,
                Box::new(definition),
            ))));
        }
        Ok(None)
    }
}
