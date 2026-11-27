// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::interaction_source::{InteractionList, WeakLink};
use super::super::shared_interaction;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const COMPONENT: &str = "SEntityComponentOutfitHangerParams";
const PORT_TYPE: &str = "TypeSubtypeParams";
const ANIMATION: &str = "SAnimatedOutfitSwapData";

/// The original equip, hang and swap capabilities of an outfit hanger.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct OutfitHanger<'source> {
    #[serde(flatten)]
    interactions: OutfitHangerInteractions<'source>,
    item_port_type_subtypes: InteractionList<OutfitHangerPortType<'source>>,
    animated_outfit_swap: SourcedValue<SourceArenaOutput<'source>>,
    animated_outfit_hang: SourcedValue<SourceArenaOutput<'source>>,
    destroy_player_items: SourcedValue<bool>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> OutfitHanger<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let interactions = OutfitHangerInteractions::read(source, notes)?;
        let item_port_type_subtypes = source.class_array_to(
            "itemPortTypeSubtypes",
            PORT_TYPE,
            notes,
            OutfitHangerPortType::read,
        )?;
        let animated_outfit_swap =
            source.raw_strong_pointer_to("animatedOutfitSwap", ANIMATION, notes)?;
        let animated_outfit_hang =
            source.raw_strong_pointer_to("animatedOutfitHang", ANIMATION, notes)?;
        let destroy_player_items = source.boolean("destroyPlayerItems", notes)?;
        let additional_fields =
            source.authored_members_except(interactions.members().into_iter().chain([
                ("itemPortTypeSubtypes", item_port_type_subtypes.state()),
                ("animatedOutfitSwap", animated_outfit_swap.state()),
                ("animatedOutfitHang", animated_outfit_hang.state()),
                ("destroyPlayerItems", destroy_player_items.state()),
            ]))?;
        Ok(Self {
            interactions,
            item_port_type_subtypes,
            animated_outfit_swap,
            animated_outfit_hang,
            destroy_player_items,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct OutfitHangerInteractions<'source> {
    #[serde(rename = "equipInteraction")]
    equip: WeakLink<'source>,
    #[serde(rename = "hangInteraction")]
    hang: WeakLink<'source>,
    #[serde(rename = "swapInteraction")]
    swap: WeakLink<'source>,
    #[serde(rename = "swapAllInteraction")]
    swap_all: WeakLink<'source>,
    #[serde(rename = "equipAllInteraction")]
    equip_all: WeakLink<'source>,
    #[serde(rename = "hangAllInteraction")]
    hang_all: WeakLink<'source>,
    #[serde(rename = "disguiseSwapAllInteraction")]
    disguise_swap_all: WeakLink<'source>,
}

impl<'source> OutfitHangerInteractions<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let mut link = |name| source.weak_link_to(name, shared_interaction::SOURCE_TYPE, notes);
        Ok(Self {
            equip: link("equipInteraction")?,
            hang: link("hangInteraction")?,
            swap: link("swapInteraction")?,
            swap_all: link("swapAllInteraction")?,
            equip_all: link("equipAllInteraction")?,
            hang_all: link("hangAllInteraction")?,
            disguise_swap_all: link("disguiseSwapAllInteraction")?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 7] {
        [
            ("equipInteraction", self.equip.state()),
            ("hangInteraction", self.hang.state()),
            ("swapInteraction", self.swap.state()),
            ("swapAllInteraction", self.swap_all.state()),
            ("equipAllInteraction", self.equip_all.state()),
            ("hangAllInteraction", self.hang_all.state()),
            ("disguiseSwapAllInteraction", self.disguise_swap_all.state()),
        ]
    }
}

/// One stored item type and subtype restriction for an outfit hanger port.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct OutfitHangerPortType<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    item_type: SourcedValue<&'source str>,
    item_sub_type: SourcedValue<&'source str>,
}

impl<'source> OutfitHangerPortType<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let item_type = source.borrowed_enum_to("itemType", "EItemType", notes)?;
        let item_sub_type = source.borrowed_enum_to("itemSubType", "EItemSubType", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("itemType", item_type.state()),
                ("itemSubType", item_sub_type.state()),
            ],
        )?;
        Ok(Self {
            source,
            item_type,
            item_sub_type,
        })
    }
}
