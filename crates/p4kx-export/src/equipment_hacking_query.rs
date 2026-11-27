// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use super::record_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_independent_configurations::hackable_configuration::HackableDevices;
use crate::catalog_v3::catalog_item_components::ItemEquipmentFamily;

pub use crate::catalog_v3::catalog_hacking_chips::{
    ChargeCount, HackingChip, HackingConfiguration, Modifier, RemovableValues,
};
pub use crate::catalog_v3::catalog_independent_configurations::hackable_configuration::{
    AudioResource, HackableAudio, HackableConfiguration, HackableSound, Rtpc,
};

record_query::item_component!(
    HackingChipRows, 'source, HackingChip<'source>, ItemEquipmentFamily::HackingChips,
    read_hacking_chips
);

record_query::unlocalized!(
    HackingChipRows,
    HackingChip,
    /// Physical Item chip components and their original source evidence.
    HackingChipSnapshot,
    with_hacking_chips,
    with_hacking_chips_from
);

equipment_component_query!(
    HackableDevices,
    HackableConfiguration,
    HackableDeviceSnapshot,
    with_hackable_devices,
    with_hackable_devices_from
);
