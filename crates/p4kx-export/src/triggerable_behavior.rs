// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_explosion::ExplosionParameters;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::authored_link::HealthLink;
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceRecordReference, SourceText};

const EXPLOSION: &str = "STriggerableDevicesBehaviorExplosionParams";
const SPAWN_ENTITY: &str = "STriggerableDevicesBehaviorSpawnEntityParams";

/// The concrete authored behavior and its shared lifecycle declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct DeviceBehavior<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    name: SourcedValue<SourceText<'source>>,
    should_be_destroyed: SourcedValue<bool>,
    start_sequence: SourcedValue<HealthLink<'source>>,
    end_sequence: SourcedValue<HealthLink<'source>>,
    #[serde(flatten)]
    effect: BehaviorEffect<'source>,
}

/// The concrete effect selected by an authored device behavior.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum BehaviorEffect<'source> {
    /// An explosion effect.
    Explosion {
        /// The authored explosion settings.
        explosion_params: Box<SourcedValue<ExplosionParameters<'source>>>,
    },
    /// An entity-spawn effect.
    SpawnEntity {
        #[schemars(with = "SourcedValue<CatalogRecordReference>")]
        /// The authored entity reference.
        entity_to_spawn: SourcedValue<Box<SourceRecordReference<'source>>>,
    },
    /// A behavior with its fields retained in the source arena.
    Other,
}

impl<'source> DeviceBehavior<'source> {
    source_getters! {
        /// Borrows the original declaration and additional fields.
        source: UnitSource<'source>,
        /// Borrows the authored name.
        name: SourcedValue<SourceText<'source>>,
        /// Borrows the authored should be destroyed.
        should_be_destroyed: SourcedValue<bool>,
        /// Borrows the authored start sequence.
        start_sequence: SourcedValue<HealthLink<'source>>,
        /// Borrows the authored end sequence.
        end_sequence: SourcedValue<HealthLink<'source>>,
        /// Borrows the authored effect.
        effect: BehaviorEffect<'source>,
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = source.source_type()?;
        let name = source
            .borrowed_string("name", notes)?
            .map(|text| SourceText::read(source_type, "name", text));
        let should_be_destroyed = source.boolean("shouldBeDestroyed", notes)?;
        let start_sequence = source.link("startSequence", notes)?;
        let end_sequence = source.link("endSequence", notes)?;
        let (effect, selected) = match source_type {
            EXPLOSION => {
                let explosion_params = source.declared_class_to(
                    "explosionParams",
                    "ExplosionParams",
                    notes,
                    ExplosionParameters::read,
                )?;
                let applied = ("explosionParams", explosion_params.state());
                (
                    BehaviorEffect::Explosion {
                        explosion_params: Box::new(explosion_params),
                    },
                    Some(applied),
                )
            }
            SPAWN_ENTITY => {
                let entity_to_spawn = source.record_link("entityToSpawn", notes)?;
                let applied = ("entityToSpawn", entity_to_spawn.state());
                (
                    BehaviorEffect::SpawnEntity { entity_to_spawn },
                    Some(applied),
                )
            }
            _ => (BehaviorEffect::Other, None),
        };
        let source = UnitSource::read(
            source,
            [
                ("name", name.state()),
                ("shouldBeDestroyed", should_be_destroyed.state()),
                ("startSequence", start_sequence.state()),
                ("endSequence", end_sequence.state()),
            ]
            .into_iter()
            .chain(selected),
        )?;
        Ok(Self {
            source,
            name,
            should_be_destroyed,
            start_sequence,
            end_sequence,
            effect,
        })
    }
}
