// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// One authored consumable effect and its text reference.
pub struct ConsumableEffect<'source> {
    #[schemars(extend("x-gameText" = true))]
    effect_description: SourcedValue<GameTextRef>,
    #[serde(flatten)]
    parameters: EffectParameters<'source>,
    #[serde(flatten)]
    source: UnitSource<'source>,
}

impl<'source> ConsumableEffect<'source> {
    /// Borrows the original protected description reference.
    #[must_use]
    pub const fn effect_description(&self) -> &SourcedValue<GameTextRef> {
        &self.effect_description
    }

    /// Borrows the original effect variant and field states.
    #[must_use]
    pub const fn parameters(&self) -> &EffectParameters<'source> {
        &self.parameters
    }

    /// Borrows the original effect type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let effect_description = source.protected_locale("effectDescription", projection, notes)?;
        let parameters = EffectParameters::read(source, notes)?;
        let source = UnitSource::read(
            source,
            std::iter::once(("effectDescription", effect_description.state()))
                .chain(parameters.projected()),
        )?;
        Ok(Self {
            effect_description,
            parameters,
            source,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
/// The original supported effect variant or its authored extension.
#[non_exhaustive]
pub enum EffectParameters<'source> {
    /// An actor status modifier.
    ModifyActorStatus {
        /// Original status selection.
        stat_type: SourcedValue<&'source str>,
        /// Authored change in status points.
        stat_point_change: SourcedValue<ResourceFloat32>,
        /// Authored change in cooldown.
        stat_cooldown_change: SourcedValue<ResourceFloat32>,
    },
    /// A health change.
    Health {
        /// Authored health change.
        health_change: SourcedValue<ResourceFloat32>,
    },
    /// A buff with optional original overrides.
    AddBuffEffect {
        /// Original buff selection.
        buff_type: SourcedValue<&'source str>,
        /// Authored duration override.
        buff_duration_override: SourcedValue<Override<'source>>,
        /// Authored value override.
        buff_value_override: SourcedValue<Override<'source>>,
    },
    /// A change to an authored resource.
    Resource {
        /// Original resource selection.
        consumable_resource_type: SourcedValue<&'source str>,
        /// Authored resource amount.
        amount: SourcedValue<ResourceFloat32>,
    },
    /// A gas reference and amount.
    Gas {
        /// Original gas and mass fields.
        gas_mass: SourcedValue<GasMass<'source>>,
    },
    /// An effect represented by the original type and extra fields.
    Authored,
}

impl<'source> EffectParameters<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "ConsumableEffectModifyActorStatus" => Self::ModifyActorStatus {
                stat_type: source.borrowed_declared_enum("statType", notes)?,
                stat_point_change: source.single("statPointChange", notes)?,
                stat_cooldown_change: source.single("statCooldownChange", notes)?,
            },
            "ConsumableEffectHealth" => Self::Health {
                health_change: source.single("healthChange", notes)?,
            },
            "ConsumableEffectAddBuffEffect" => Self::AddBuffEffect {
                buff_type: source.borrowed_declared_enum("buffType", notes)?,
                buff_duration_override: source.declared_pointer(
                    "buffDurationOverride",
                    notes,
                    Override::duration,
                )?,
                buff_value_override: source.declared_pointer(
                    "buffValueOverride",
                    notes,
                    Override::value,
                )?,
            },
            "ConsumableEffectResource" => Self::Resource {
                consumable_resource_type: source
                    .borrowed_declared_enum("consumableResourceType", notes)?,
                amount: source.single("amount", notes)?,
            },
            "ConsumableEffectGas" => Self::Gas {
                gas_mass: source.declared_class("gasMass", notes, GasMass::read)?,
            },
            _ => Self::Authored,
        })
    }

    fn projected(&self) -> impl Iterator<Item = (&'static str, p4kx_extraction::SourceValueState)> {
        let fields = match self {
            Self::ModifyActorStatus {
                stat_type,
                stat_point_change,
                stat_cooldown_change,
            } => [
                Some(("statType", stat_type.state())),
                Some(("statPointChange", stat_point_change.state())),
                Some(("statCooldownChange", stat_cooldown_change.state())),
            ],
            Self::Health { health_change } => {
                [Some(("healthChange", health_change.state())), None, None]
            }
            Self::AddBuffEffect {
                buff_type,
                buff_duration_override,
                buff_value_override,
            } => [
                Some(("buffType", buff_type.state())),
                Some(("buffDurationOverride", buff_duration_override.state())),
                Some(("buffValueOverride", buff_value_override.state())),
            ],
            Self::Resource {
                consumable_resource_type,
                amount,
            } => [
                Some(("consumableResourceType", consumable_resource_type.state())),
                Some(("amount", amount.state())),
                None,
            ],
            Self::Gas { gas_mass } => [Some(("gasMass", gas_mass.state())), None, None],
            Self::Authored => [None, None, None],
        };
        fields.into_iter().flatten()
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// An original gas reference and mass.
pub struct GasMass<'source> {
    #[schemars(with = "SourcedValue<crate::catalog_record_reference::CatalogRecordReference>")]
    gas: SourcedValue<Box<crate::source_value_catalog::SourceRecordReference<'source>>>,
    mass: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    source: UnitSource<'source>,
}

impl<'source> GasMass<'source> {
    /// Borrows the original gas Reference.
    #[must_use]
    pub const fn gas(
        &self,
    ) -> &SourcedValue<Box<crate::source_value_catalog::SourceRecordReference<'source>>> {
        &self.gas
    }

    /// Borrows the original gas mass.
    #[must_use]
    pub const fn mass(&self) -> &SourcedValue<ResourceFloat32> {
        &self.mass
    }

    /// Borrows the original gas mass type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let gas = source.record_link("Gas", notes)?;
        let mass = source.single("Mass", notes)?;
        let source = UnitSource::read(source, [("Gas", gas.state()), ("Mass", mass.state())])?;
        Ok(Self { gas, mass, source })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// An original numeric buff override.
pub struct Override<'source> {
    amount: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    source: UnitSource<'source>,
}

impl<'source> Override<'source> {
    /// Borrows the original numeric override.
    #[must_use]
    pub const fn amount(&self) -> &SourcedValue<ResourceFloat32> {
        &self.amount
    }

    /// Borrows the original override type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    fn duration(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Self::read(source, "durationOverride", notes)
    }

    fn value(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Self::read(source, "valueOverride", notes)
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        field: &'static str,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let amount = source.single(field, notes)?;
        let source = UnitSource::read(source, [(field, amount.state())])?;
        Ok(Self { amount, source })
    }
}
