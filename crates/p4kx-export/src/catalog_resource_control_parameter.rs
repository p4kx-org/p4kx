// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const BASE: &str = "ItemResourceControlParameterBase";
const PARAMETER: &str = "ResourceNetworkAcessParameter";

/// An authored resource control parameter and its optional external port declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum ResourceControlParameter<'source> {
    /// A parameter from the resource network.
    NetworkParameter {
        #[serde(flatten)]
        /// Original declaration and position.
        source: ParameterSource<'source>,
        /// Authored network parameter label.
        parameter: SourcedValue<&'source str>,
    },
    /// A resource parameter with an external port declaration.
    ExternalPort {
        #[serde(flatten)]
        /// Original declaration and position.
        source: ParameterSource<'source>,
        /// Authored network parameter label.
        parameter: SourcedValue<&'source str>,
        /// Authored parameter name.
        name: SourcedValue<&'source str>,
        /// Authored external port name.
        port_name: SourcedValue<&'source str>,
    },
    /// An original subtype retained as authored fields.
    UnknownParameter {
        #[serde(flatten)]
        /// Original declaration and position.
        source: ParameterSource<'source>,
    },
}

/// Identity and original fields of a control parameter.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct ParameterSource<'source> {
    /// Original DataForge declaration.
    pub source_type: SourcedValue<&'source str>,
    /// Zero-based position in the original array.
    pub source_position: SourcedValue<u32>,
    /// Original fields outside the typed projection.
    pub additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceControlParameter<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            "ItemResourceControlParameterRNParameter" => {
                let parameter = source.borrowed_enum_to("parameter", PARAMETER, notes)?;
                Self::NetworkParameter {
                    source: ParameterSource::read(source, [("parameter", parameter.state())])?,
                    parameter,
                }
            }
            "ItemResourceControlRNParameterExternalPort" => {
                let parameter = source.borrowed_enum_to("parameter", PARAMETER, notes)?;
                let name = source.borrowed_string("name", notes)?;
                let port_name = source.borrowed_string("portName", notes)?;
                Self::ExternalPort {
                    source: ParameterSource::read(
                        source,
                        [
                            ("parameter", parameter.state()),
                            ("name", name.state()),
                            ("portName", port_name.state()),
                        ],
                    )?,
                    parameter,
                    name,
                    port_name,
                }
            }
            _ => Self::UnknownParameter {
                source: ParameterSource::read(source, [])?,
            },
        })
    }
}

impl<'source> ParameterSource<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projected: impl IntoIterator<Item = (&'static str, SourceValueState)>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source_type: source.source_type_value()?,
            source_position: source.array_ordinal()?,
            additional_fields: source.authored_members_except(projected)?,
        })
    }
}
