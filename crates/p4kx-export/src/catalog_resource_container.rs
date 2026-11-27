// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::cargo_source;
use super::catalog_capacity::ResourceCapacity;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const RESOURCE_CONTAINER: &str = "ResourceContainer";

/// Authored contents and policy of a physical `ResourceContainer` component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ResourceContainer<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    /// Original DataForge declaration.
    pub source_type: SourcedValue<&'source str>,
    /// Authored capacity in its original unit and numeric representation.
    pub capacity: SourcedValue<ResourceCapacity<'source>>,
    /// Whether content changes are disabled.
    pub immutable: SourcedValue<bool>,
    /// Original mutability level label.
    pub mutability_level: SourcedValue<&'source str>,
    /// Whether quality generation is enabled.
    pub generate_random_quality: SourcedValue<bool>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    /// Ordered original resource allowlist, including null and dangling links.
    pub inclusive_resources: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    /// Ordered original resource denylist, including null and dangling links.
    pub exclusive_resources: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    /// Ordered original resource group allowlist.
    pub inclusive_groups: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    /// Ordered original resource group denylist.
    pub exclusive_groups: SourcedValue<Vec<SourcedValue<Box<SourceRecordReference<'source>>>>>,
    /// Ordered original default composition slots.
    pub default_composition: SourcedValue<Vec<SourcedValue<CompositionEntry<'source>>>>,
    /// Authored default composition fill factor.
    pub default_composition_fill_factor: SourcedValue<ResourceFloat32>,
    /// Whether this container is hidden in the UI.
    pub hide_from_ui: SourcedValue<bool>,
    /// Original fields outside the typed projection.
    pub additional_fields: SourceArenaOutput<'source>,
}

/// One physical slot in a container default composition.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CompositionEntry<'source> {
    /// Original DataForge declaration.
    pub source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    /// Original resource reference and target disposition.
    pub entry: SourcedValue<Box<SourceRecordReference<'source>>>,
    /// Authored composition weight.
    pub weight: SourcedValue<ResourceFloat32>,
    /// Original fields outside the typed projection.
    pub additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceContainer<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let capacity = source.strong_pointer_family_to(
            "capacity",
            cargo_source::BASE,
            notes,
            ResourceCapacity::read,
        )?;
        let immutable = source.boolean("immutable", notes)?;
        let mutability_level = source.borrowed_enum_to(
            "mutabilityLevel",
            "EResourceContainerMutabilityLevel",
            notes,
        )?;
        let generate_random_quality = source.boolean("generateRandomQuality", notes)?;
        let inclusive_resources =
            source.record_link_array_to("inclusiveResources", "ResourceType", notes)?;
        let exclusive_resources =
            source.record_link_array_to("exclusiveResources", "ResourceType", notes)?;
        let inclusive_groups =
            source.record_link_array_to("inclusiveGroups", "ResourceTypeGroup", notes)?;
        let exclusive_groups =
            source.record_link_array_to("exclusiveGroups", "ResourceTypeGroup", notes)?;
        let default_composition =
            source.declared_owned_array("defaultComposition", notes, CompositionEntry::read)?;
        let default_composition_fill_factor =
            source.single("defaultCompositionFillFactor", notes)?;
        let hide_from_ui = source.boolean("hideFromUI", notes)?;
        let additional_fields = source.authored_members_except([
            ("capacity", capacity.state()),
            ("immutable", immutable.state()),
            ("mutabilityLevel", mutability_level.state()),
            ("generateRandomQuality", generate_random_quality.state()),
            ("inclusiveResources", inclusive_resources.state()),
            ("exclusiveResources", exclusive_resources.state()),
            ("inclusiveGroups", inclusive_groups.state()),
            ("exclusiveGroups", exclusive_groups.state()),
            ("defaultComposition", default_composition.state()),
            (
                "defaultCompositionFillFactor",
                default_composition_fill_factor.state(),
            ),
            ("hideFromUI", hide_from_ui.state()),
        ])?;
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            capacity,
            immutable,
            mutability_level,
            generate_random_quality,
            inclusive_resources,
            exclusive_resources,
            inclusive_groups,
            exclusive_groups,
            default_composition,
            default_composition_fill_factor,
            hide_from_ui,
            additional_fields,
        })
    }

    /// Returns the original component owner.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}

impl<'source> CompositionEntry<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let entry = source.record_or_weak_record_link("entry", notes)?;
        let weight = source.single("weight", notes)?;
        let additional_fields = source
            .authored_members_except([("entry", entry.state()), ("weight", weight.state())])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            entry,
            weight,
            additional_fields,
        })
    }
}
