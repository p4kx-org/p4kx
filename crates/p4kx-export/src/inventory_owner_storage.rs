// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::inventory_container_definition;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference, SourceWeakReference};

pub(super) const ITEM_COMPONENT: &str = "SCItemInventoryContainerComponentParams";
pub(super) const VEHICLE_FIELD: &str = "inventoryContainerParams";
const INTERACTION: &str = "SSharedInteractionParams";

type ContainerLink<'source> = SourcedValue<Box<SourceRecordReference<'source>>>;
type InteractionLink<'source> = SourcedValue<Box<SourceWeakReference<'source>>>;

/// One physical owner's authored reference to a storage definition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InventoryOwnerStorage<'source> {
    #[serde(flatten)]
    pub(super) owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    fields: StorageFields<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum StorageFields<'source> {
    ItemStorage(Box<ItemStorage<'source>>),
    VehicleStorage {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        inventory_container_params: ContainerLink<'source>,
    },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ItemStorage<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    container_params: ContainerLink<'source>,
    open_inventory_interaction: InteractionLink<'source>,
    open_looting_interaction: InteractionLink<'source>,
    can_auto_store_restricted_items: SourcedValue<bool>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> InventoryOwnerStorage<'source> {
    pub(super) fn item(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            fields: StorageFields::ItemStorage(Box::new(ItemStorage::read(source, notes)?)),
        })
    }

    pub(super) fn vehicle(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            fields: StorageFields::VehicleStorage {
                inventory_container_params: source.record_link_to(
                    VEHICLE_FIELD,
                    inventory_container_definition::SOURCE_TYPE,
                    notes,
                )?,
            },
        })
    }
}

impl<'source> ItemStorage<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let container_params = source.record_link_to(
            "containerParams",
            inventory_container_definition::SOURCE_TYPE,
            notes,
        )?;
        let open_inventory_interaction =
            source.weak_link_to("openInventoryInteraction", INTERACTION, notes)?;
        let open_looting_interaction =
            source.weak_link_to("openLootingInteraction", INTERACTION, notes)?;
        let can_auto_store_restricted_items =
            source.boolean("canAutoStoreRestrictedItems", notes)?;
        let additional_fields = source.authored_members_except([
            ("containerParams", container_params.state()),
            (
                "openInventoryInteraction",
                open_inventory_interaction.state(),
            ),
            ("openLootingInteraction", open_looting_interaction.state()),
            (
                "canAutoStoreRestrictedItems",
                can_auto_store_restricted_items.state(),
            ),
        ])?;
        Ok(Self {
            container_params,
            open_inventory_interaction,
            open_looting_interaction,
            can_auto_store_restricted_items,
            additional_fields,
        })
    }
}
