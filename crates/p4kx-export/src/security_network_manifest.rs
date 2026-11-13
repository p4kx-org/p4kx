// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use self::security_network_protocol::{
    HostilityRules, SecurityNetworkProtocol, SecurityProtocolOverride,
};
use self::security_network_variables::{SecurityNetworkVariable, SecurityVariableEffects};
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::interaction_source::InteractionList;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

#[path = "security_network_protocol.rs"]
mod security_network_protocol;
#[path = "security_network_variables.rs"]
mod security_network_variables;

pub(super) const SOURCE_TYPE: &str = "SecurityNetworkManifest";

/// A physical security network with its authored protocols, hostility rules and effects.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SecurityNetworkManifest<'source> {
    #[serde(flatten)]
    source: CanonicalRecordHeader<'source>,
    default_protocol: SourcedValue<SecurityNetworkProtocol<'source>>,
    room_protocols: InteractionList<SecurityProtocolOverride<'source>>,
    variables: InteractionList<SecurityNetworkVariable<'source>>,
    hostility_rules: InteractionList<HostilityRules<'source>>,
    variable_effects: InteractionList<SecurityVariableEffects<'source>>,
    is_neutral_territory: SourcedValue<bool>,
    teleport_player_out: SourcedValue<bool>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> SecurityNetworkManifest<'source> {
    pub(super) fn read(
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        _projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let default_protocol =
            SecurityNetworkProtocol::read_field(fields, "defaultProtocol", notes)?;
        let room_protocols = SecurityProtocolOverride::read_array(fields, notes)?;
        let variables = SecurityNetworkVariable::read_array(fields, notes)?;
        let hostility_rules = HostilityRules::read_array(fields, notes)?;
        let variable_effects = SecurityVariableEffects::read_array(fields, notes)?;
        let is_neutral_territory = fields.boolean("isNeutralTerritory", notes)?;
        let teleport_player_out = fields.boolean("teleportPlayerOut", notes)?;
        let additional_fields = fields.authored_members_except([
            ("defaultProtocol", default_protocol.state()),
            ("roomProtocols", room_protocols.state()),
            ("variables", variables.state()),
            ("hostilityRules", hostility_rules.state()),
            ("variableEffects", variable_effects.state()),
            ("isNeutralTerritory", is_neutral_territory.state()),
            ("teleportPlayerOut", teleport_player_out.state()),
        ])?;
        Ok(Self {
            source,
            default_protocol,
            room_protocols,
            variables,
            hostility_rules,
            variable_effects,
            is_neutral_territory,
            teleport_player_out,
            additional_fields,
        })
    }
}
