// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Collected;
use crate::guid_text::{CatalogGuid, sort_by_guid};

impl Collected<'_> {
    /// Orders the failures and the picked kinds by the text of the GUID.
    pub(super) fn order(&mut self) {
        self.order_failures();
        self.explosion_definitions
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        self.ammo_definitions
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        sort_by_guid(&mut self.manufacturers, |entity| entity.guid);
        self.factions
            .sort_by_cached_key(|row| (CatalogGuid(row.physical_guid).text(), row.physical_index));
        sort_by_guid(&mut self.items, |entity| entity.guid);
        sort_by_guid(&mut self.ships, |entity| entity.guid);
        sort_by_guid(&mut self.commodities, |entity| entity.guid);
        self.resource_groups
            .sort_by_cached_key(|row| (row.guid.text(), row.record_index));
        sort_by_guid(&mut self.resource_types, |entity| entity.guid);
        sort_by_guid(&mut self.blueprint_categories, |entity| entity.guid);
        sort_by_guid(&mut self.blueprint_category_lists, |entity| entity.guid);
        sort_by_guid(&mut self.starmap_objects, |entity| entity.guid);
        sort_by_guid(&mut self.mineable_elements, |entity| entity.guid);
        sort_by_guid(&mut self.refining_processes, |entity| entity.guid);
        sort_by_guid(&mut self.mineable_compositions, |entity| entity.guid);
        sort_by_guid(&mut self.mining_global_params, |entity| entity.guid);
        sort_by_guid(&mut self.mineable_entities, |entity| entity.guid);
        sort_by_guid(&mut self.crafters, |entity| entity.guid);
        sort_by_guid(&mut self.harvestable_provider_presets, |entity| entity.guid);
        sort_by_guid(&mut self.sub_harvestable_multi_configs, |entity| {
            entity.guid
        });
        self.mission_kinds.order();
        self.loot_kinds.order();
        self.tags.order_entities();
    }
}
