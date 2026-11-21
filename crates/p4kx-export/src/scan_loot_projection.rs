// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, TreeBuilder};

use super::SelectedProjection;
use crate::catalog_v3::loot_definition_catalog::{self, LootDefinitionKind};
use crate::harvest_config::{is_slotpreset_path, multi_config};
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
            let value = loot_definition_catalog::read(
                admission.record(),
                tree,
                self.projection.original(),
                self.scan.options.cancel,
            )?;
            let definition = Box::new(value);
            let picked = if kind == Kind::SubHarvestableMultiConfigs {
                let path = tree.record_path();
                let projection = (is_slotpreset_path(path)
                    && !crate::selection::is_leftover(path, tree.name()))
                .then(|| multi_config(trees, tree, self.scan.forge, &self.scan.links));
                Picked::MultiConfig {
                    definition,
                    projection,
                }
            } else {
                Picked::Loot(LootPick::Definition(loot, definition))
            };
            return Ok(Some(picked));
        }
        Ok(None)
    }
}
