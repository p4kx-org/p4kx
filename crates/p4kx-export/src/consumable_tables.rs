// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use super::super::component_sources::physical_component;
use super::super::referenced_definition_read::{self, Selection};
use super::{ComponentKind, PhysicalComponents};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::{CatalogProjection, TextOwner};
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::catalog_text_contract::PublicNameRef;
use crate::export_field_inspection::ConfigurationInspection;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

#[path = "consumable_component.rs"]
mod component;
#[path = "consumable_effect.rs"]
mod effect;

pub use component::AuthoredList;
pub(crate) use component::SUBTYPE;
pub use component::{Consumable, Contents as ConsumableContents};
pub use effect::{ConsumableEffect, EffectParameters, GasMass, Override};

pub(crate) const TYPE: &str = "ConsumableType";
const CONSUMABLES: ComponentKind = ComponentKind {
    source_type: component::COMPONENT,
    inspection: ConfigurationInspection::ConsumableComponent,
};

physical_component!(ConsumableComponents, Consumable, CONSUMABLES);

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ConsumableDefinitions<'source> {
    consumable_types: CatalogTable<ConsumableType<'source>>,
    consumable_subtypes: CatalogTable<ConsumableSubtype<'source>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ConsumableConfigurations<'source> {
    consumables: CatalogTable<Consumable<'source>>,
}

pub(super) struct ConsumableTables<'source> {
    pub(super) configurations: ConsumableConfigurations<'source>,
    pub(super) definitions: ConsumableDefinitions<'source>,
}

impl<'source> ConsumableTables<'source> {
    pub(super) fn read(
        components: &mut PhysicalComponents<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        let consumables = components.table(CONSUMABLES, Consumable::read)?;
        let mut linked = consumables
            .rows
            .iter()
            .flat_map(Consumable::subtype_indices)
            .collect::<BTreeSet<_>>();
        let selection = Selection {
            source_types: &[TYPE],
            linked: &BTreeSet::new(),
            filter: components.options.filter,
            inspection: ConfigurationInspection::ConsumableDefinition,
        };
        let types = referenced_definition_read::read(
            components.forge,
            components.projection.original(),
            &selection,
            components.options.cancel,
            ConsumableType::read,
        )?;
        let consumable_types = CatalogTable::new(
            components.forge,
            &[TYPE],
            components.keep_definitions(types),
        );
        linked.extend(
            consumable_types
                .rows
                .iter()
                .flat_map(ConsumableType::subtype_indices),
        );
        let projection = components
            .projection
            .for_text_owner(TextOwner::ConsumableSubtypes);
        let subtypes = referenced_definition_read::read(
            components.forge,
            components.projection.original(),
            &Selection {
                source_types: &[SUBTYPE],
                linked: &linked,
                ..selection
            },
            components.options.cancel,
            |_, header, source, notes| ConsumableSubtype::read(header, source, &projection, notes),
        )?;
        let consumable_subtypes = CatalogTable::new(
            components.forge,
            &[SUBTYPE],
            components.keep_definitions(subtypes),
        );
        Ok(Self {
            configurations: ConsumableConfigurations { consumables },
            definitions: ConsumableDefinitions {
                consumable_types,
                consumable_subtypes,
            },
        })
    }
}

/// One physical consumable type and its authored subtype slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ConsumableType<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    type_name: SourcedValue<&'source str>,
    #[schemars(with = "AuthoredList<CatalogRecordReference>")]
    subtypes: AuthoredList<Box<SourceRecordReference<'source>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ConsumableType<'source> {
    /// Borrows the authored type name.
    #[must_use]
    pub const fn type_name(&self) -> &SourcedValue<&'source str> {
        &self.type_name
    }

    /// Borrows ordered authored subtype Reference slots.
    #[must_use]
    pub const fn subtypes(&self) -> &AuthoredList<Box<SourceRecordReference<'source>>> {
        &self.subtypes
    }

    /// Borrows remaining original fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    /// Borrows the physical definition header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    pub(in crate::catalog_v3) fn read(
        _kind: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let type_name = fields.borrowed_string("typeName", notes)?;
        let subtypes = fields.record_link_array_to("subtypes", SUBTYPE, notes)?;
        let additional_fields = fields.authored_members_except([
            ("typeName", type_name.state()),
            ("subtypes", subtypes.state()),
        ])?;
        Ok(Self {
            source,
            type_name,
            subtypes,
            additional_fields,
        })
    }

    fn subtype_indices(&self) -> impl Iterator<Item = u32> + '_ {
        self.subtypes
            .value()
            .into_iter()
            .flatten()
            .filter_map(|slot| slot.value()?.resolved_index())
    }
}

/// One physical consumable subtype and its authored effects.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ConsumableSubtype<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    type_name: SourcedValue<&'source str>,
    consumable_name: SourcedValue<PublicNameRef>,
    #[serde(rename = "effectsPerMicroSCU")]
    effects: AuthoredList<ConsumableEffect<'source>>,
    tint_override: SourcedValue<UnitSource<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ConsumableSubtype<'source> {
    /// Borrows the authored subtype name.
    #[must_use]
    pub const fn type_name(&self) -> &SourcedValue<&'source str> {
        &self.type_name
    }

    /// Borrows the original public-name reference.
    #[must_use]
    pub const fn consumable_name(&self) -> &SourcedValue<PublicNameRef> {
        &self.consumable_name
    }

    /// Borrows ordered authored effect slots.
    #[must_use]
    pub const fn effects(&self) -> &AuthoredList<ConsumableEffect<'source>> {
        &self.effects
    }

    /// Borrows the original pointed tint type and fields.
    #[must_use]
    pub const fn tint_override(&self) -> &SourcedValue<UnitSource<'source>> {
        &self.tint_override
    }

    /// Borrows remaining original fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    /// Borrows the physical definition header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    pub(in crate::catalog_v3) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let type_name = fields.borrowed_string("typeName", notes)?;
        let consumable_name = fields.public_name("consumableName", projection, notes)?;
        let effects =
            fields.declared_family_array("effectsPerMicroSCU", notes, |effect, notes| {
                ConsumableEffect::read(effect, projection, notes)
            })?;
        let tint_override =
            fields.declared_pointer("tintOverride", notes, |tint, _| UnitSource::read(tint, []))?;
        let additional_fields = fields.authored_members_except([
            ("typeName", type_name.state()),
            ("consumableName", consumable_name.state()),
            ("effectsPerMicroSCU", effects.state()),
            ("tintOverride", tint_override.state()),
        ])?;
        Ok(Self {
            source,
            type_name,
            consumable_name,
            effects,
            tint_override,
            additional_fields,
        })
    }
}
