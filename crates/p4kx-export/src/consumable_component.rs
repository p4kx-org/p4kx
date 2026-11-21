// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::catalog_capacity::ResourceCapacity;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

pub(super) const COMPONENT: &str = "SCItemConsumableParams";
pub(crate) const SUBTYPE: &str = "ConsumableSubtype";

/// An original list and its individually sourced slots.
pub type AuthoredList<T> = SourcedValue<Vec<SourcedValue<T>>>;

/// One original consumable component and its authored contents and use settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct Consumable<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    fields: ConsumableFields<'source>,
    #[serde(flatten)]
    source: UnitSource<'source>,
}

impl<'source> Consumable<'source> {
    /// Borrows the original physical component owner.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original contents slots in order.
    #[must_use]
    pub const fn default_contents(&self) -> &AuthoredList<Contents<'source>> {
        &self.fields.default_contents
    }

    /// Borrows the original consumable capacity in its stored unit.
    #[must_use]
    pub const fn consumable_volume(&self) -> &SourcedValue<ResourceCapacity<'source>> {
        &self.fields.consumable_volume
    }

    /// Borrows the authored container type tag.
    #[must_use]
    pub const fn container_type_tag(&self) -> &SourcedValue<&'source str> {
        &self.fields.container_type_tag
    }

    /// Borrows the authored lid type tag.
    #[must_use]
    pub const fn lid_type_tag(&self) -> &SourcedValue<&'source str> {
        &self.fields.lid_type_tag
    }

    /// Borrows the authored utensil type tag.
    #[must_use]
    pub const fn utensil_type_tag(&self) -> &SourcedValue<&'source str> {
        &self.fields.utensil_type_tag
    }

    /// Borrows the authored container state.
    #[must_use]
    pub const fn container_closed(&self) -> &SourcedValue<bool> {
        &self.fields.container_closed
    }

    /// Borrows the flag that permits reclosing the container.
    #[must_use]
    pub const fn can_be_reclosed(&self) -> &SourcedValue<bool> {
        &self.fields.can_be_reclosed
    }

    /// Borrows the authored discard-on-consumption flag.
    #[must_use]
    pub const fn discard_when_consumed(&self) -> &SourcedValue<bool> {
        &self.fields.discard_when_consumed
    }

    /// Borrows the authored one-shot consumption flag.
    #[must_use]
    pub const fn one_shot_consume(&self) -> &SourcedValue<bool> {
        &self.fields.one_shot_consume
    }

    /// Borrows the flag that permits prone consumption.
    #[must_use]
    pub const fn can_consume_in_prone(&self) -> &SourcedValue<bool> {
        &self.fields.can_consume_in_prone
    }

    /// Borrows the flag that permits skipping the animation loop.
    #[must_use]
    pub const fn can_skip_consume_anim_loop(&self) -> &SourcedValue<bool> {
        &self.fields.can_skip_consume_anim_loop
    }

    /// Borrows the authored prop animation flag.
    #[must_use]
    pub const fn is_prop_animated(&self) -> &SourcedValue<bool> {
        &self.fields.is_prop_animated
    }

    /// Borrows the authored two-handed consumption flag.
    #[must_use]
    pub const fn is_two_handed_consume(&self) -> &SourcedValue<bool> {
        &self.fields.is_two_handed_consume
    }

    /// Borrows the original type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    pub(super) fn read(
        owner: ComponentOwner,
        input: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let fields = ConsumableFields::read(input, notes)?;
        let source = UnitSource::read(input, fields.projected())?;
        Ok(Self {
            owner,
            fields,
            source,
        })
    }

    pub(super) fn subtype_indices(&self) -> impl Iterator<Item = u32> + '_ {
        self.fields
            .default_contents
            .value()
            .into_iter()
            .flatten()
            .filter_map(|slot| slot.value()?.consumable_subtype.value()?.resolved_index())
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct ConsumableFields<'source> {
    default_contents: AuthoredList<Contents<'source>>,
    consumable_volume: SourcedValue<ResourceCapacity<'source>>,
    container_type_tag: SourcedValue<&'source str>,
    lid_type_tag: SourcedValue<&'source str>,
    utensil_type_tag: SourcedValue<&'source str>,
    container_closed: SourcedValue<bool>,
    can_be_reclosed: SourcedValue<bool>,
    discard_when_consumed: SourcedValue<bool>,
    one_shot_consume: SourcedValue<bool>,
    can_consume_in_prone: SourcedValue<bool>,
    can_skip_consume_anim_loop: SourcedValue<bool>,
    is_prop_animated: SourcedValue<bool>,
    is_two_handed_consume: SourcedValue<bool>,
}

impl<'source> ConsumableFields<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            default_contents: source.declared_class_array(
                "defaultContents",
                notes,
                Contents::read,
            )?,
            consumable_volume: source.declared_pointer(
                "consumableVolume",
                notes,
                ResourceCapacity::read,
            )?,
            container_type_tag: source.borrowed_string("containerTypeTag", notes)?,
            lid_type_tag: source.borrowed_string("lidTypeTag", notes)?,
            utensil_type_tag: source.borrowed_string("utensilTypeTag", notes)?,
            container_closed: source.boolean("containerClosed", notes)?,
            can_be_reclosed: source.boolean("canBeReclosed", notes)?,
            discard_when_consumed: source.boolean("discardWhenConsumed", notes)?,
            one_shot_consume: source.boolean("oneShotConsume", notes)?,
            can_consume_in_prone: source.boolean("canConsumeInProne", notes)?,
            can_skip_consume_anim_loop: source.boolean("canSkipConsumeAnimLoop", notes)?,
            is_prop_animated: source.boolean("isPropAnimated", notes)?,
            is_two_handed_consume: source.boolean("isTwoHandedConsume", notes)?,
        })
    }

    fn projected(
        &self,
    ) -> impl IntoIterator<Item = (&'static str, p4kx_extraction::SourceValueState)> {
        [
            ("defaultContents", self.default_contents.state()),
            ("consumableVolume", self.consumable_volume.state()),
            ("containerTypeTag", self.container_type_tag.state()),
            ("lidTypeTag", self.lid_type_tag.state()),
            ("utensilTypeTag", self.utensil_type_tag.state()),
            ("containerClosed", self.container_closed.state()),
            ("canBeReclosed", self.can_be_reclosed.state()),
            ("discardWhenConsumed", self.discard_when_consumed.state()),
            ("oneShotConsume", self.one_shot_consume.state()),
            ("canConsumeInProne", self.can_consume_in_prone.state()),
            (
                "canSkipConsumeAnimLoop",
                self.can_skip_consume_anim_loop.state(),
            ),
            ("isPropAnimated", self.is_prop_animated.state()),
            ("isTwoHandedConsume", self.is_two_handed_consume.state()),
        ]
    }
}

/// An original consumable contents slot, with its subtype link and ratio.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct Contents<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    consumable_subtype: SourcedValue<Box<SourceRecordReference<'source>>>,
    ratio: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    source: UnitSource<'source>,
}

impl<'source> Contents<'source> {
    /// Borrows the original physical subtype reference.
    #[must_use]
    pub const fn consumable_subtype(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.consumable_subtype
    }

    /// Borrows the original ratio preserving the stored bits.
    #[must_use]
    pub const fn ratio(&self) -> &SourcedValue<ResourceFloat32> {
        &self.ratio
    }

    /// Borrows the original type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    fn read(
        input: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let consumable_subtype = input.record_link_to("consumableSubtype", SUBTYPE, notes)?;
        let ratio = input.single("ratio", notes)?;
        let source = UnitSource::read(
            input,
            [
                ("consumableSubtype", consumable_subtype.state()),
                ("ratio", ratio.state()),
            ],
        )?;
        Ok(Self {
            consumable_subtype,
            ratio,
            source,
        })
    }
}
