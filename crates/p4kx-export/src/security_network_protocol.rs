// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::interaction_source::{InteractionList, RecordLink, RecordLinks};
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const PROTOCOL_TYPE: &str = "SecurityNetworkProtocol";
const PERMISSIONS_TYPE: &str = "SecurityNetworkPermissions";
const PERMISSION_VALUE: &str = "ESecurityNetworkPermissionValue";
const CLEARANCE_TYPE: &str = "SecurityClearanceConditions";
const CLEARANCE_TOKEN: &str = "SecurityClearanceToken";
const PROTOCOL_OVERRIDE: &str = "SecurityNetworkProtocolOverride";
const HOSTILITY_TYPE: &str = "SHostilityRules";
const FACTION: &str = "Faction";
const LEGACY_FACTION: &str = "Faction_LEGACY";
const TAG: &str = "Tag";
const TAG_LIST: &str = "TagList";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SecurityNetworkProtocol<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    permissions_with_clearance: SourcedValue<SecurityPermissions<'source>>,
    permissions_without_clearance: SourcedValue<SecurityPermissions<'source>>,
    accepted_clearance: SourcedValue<SecurityClearance<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    owner_faction: RecordLink<'source>,
}

impl<'source> SecurityNetworkProtocol<'source> {
    pub(super) fn read_field(
        fields: Observed<'_, '_, '_, 'source>,
        name: &str,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to(name, PROTOCOL_TYPE, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let permissions_with_clearance = fields.declared_class_to(
            "permissionsWithClearance",
            PERMISSIONS_TYPE,
            notes,
            SecurityPermissions::read,
        )?;
        let permissions_without_clearance = fields.declared_class_to(
            "permissionsWithoutClearance",
            PERMISSIONS_TYPE,
            notes,
            SecurityPermissions::read,
        )?;
        let accepted_clearance = fields.declared_class_to(
            "acceptedClearance",
            CLEARANCE_TYPE,
            notes,
            SecurityClearance::read,
        )?;
        let owner_faction = fields.record_link_to("ownerFaction", FACTION, notes)?;
        let source = UnitSource::read(
            fields,
            [
                (
                    "permissionsWithClearance",
                    permissions_with_clearance.state(),
                ),
                (
                    "permissionsWithoutClearance",
                    permissions_without_clearance.state(),
                ),
                ("acceptedClearance", accepted_clearance.state()),
                ("ownerFaction", owner_faction.state()),
            ],
        )?;
        Ok(Self {
            source,
            permissions_with_clearance,
            permissions_without_clearance,
            accepted_clearance,
            owner_faction,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SecurityPermissions<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    access: SourcedValue<&'source str>,
    trespass: SourcedValue<&'source str>,
}

impl<'source> SecurityPermissions<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let access = fields.borrowed_enum_to("access", PERMISSION_VALUE, notes)?;
        let trespass = fields.borrowed_enum_to("trespass", PERMISSION_VALUE, notes)?;
        let source = UnitSource::read(
            fields,
            [("access", access.state()), ("trespass", trespass.state())],
        )?;
        Ok(Self {
            source,
            access,
            trespass,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct SecurityClearance<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tokens: RecordLinks<'source>,
}

impl<'source> SecurityClearance<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tokens = fields.record_link_array_to("tokens", CLEARANCE_TOKEN, notes)?;
        let source = UnitSource::read(fields, [("tokens", tokens.state())])?;
        Ok(Self { source, tokens })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct SecurityProtocolOverride<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    room_identifiers: SourcedValue<RoomIdentifiers<'source>>,
    protocol: SourcedValue<SecurityNetworkProtocol<'source>>,
}

impl<'source> SecurityProtocolOverride<'source> {
    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<InteractionList<Self>, CatalogError> {
        fields.class_array_to("roomProtocols", PROTOCOL_OVERRIDE, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let room_identifiers =
            fields.declared_class_to("roomIdentifiers", TAG_LIST, notes, RoomIdentifiers::read)?;
        let protocol = SecurityNetworkProtocol::read_field(fields, "protocol", notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("roomIdentifiers", room_identifiers.state()),
                ("protocol", protocol.state()),
            ],
        )?;
        Ok(Self {
            source,
            room_identifiers,
            protocol,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct RoomIdentifiers<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<Vec<SourcedValue<CatalogRecordReference>>>")]
    tags: RecordLinks<'source>,
}

impl<'source> RoomIdentifiers<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tags = fields.record_link_array_to("tags", TAG, notes)?;
        let source = UnitSource::read(fields, [("tags", tags.state())])?;
        Ok(Self { source, tags })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct HostilityRules<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    tag: RecordLink<'source>,
    use_reputation_hostility: SourcedValue<bool>,
    use_personal_hostility: SourcedValue<bool>,
    use_faction_hostility: SourcedValue<bool>,
    polices_lawful_trespass: SourcedValue<bool>,
    hostile_to_criminals: SourcedValue<bool>,
    hostile_to_stolen_vehicles: SourcedValue<bool>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    faction_override: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    faction_to_override: RecordLink<'source>,
}

impl<'source> HostilityRules<'source> {
    pub(super) fn read_array(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<InteractionList<Self>, CatalogError> {
        fields.class_array_to("hostilityRules", HOSTILITY_TYPE, notes, Self::read)
    }

    pub(super) fn read_effect(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to("rules", HOSTILITY_TYPE, notes, Self::read)
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let tag = fields.record_link_to("tag", TAG, notes)?;
        let use_reputation_hostility = fields.boolean("useReputationHostility", notes)?;
        let use_personal_hostility = fields.boolean("usePersonalHostility", notes)?;
        let use_faction_hostility = fields.boolean("useFactionHostility", notes)?;
        let polices_lawful_trespass = fields.boolean("policesLawfulTrespass", notes)?;
        let hostile_to_criminals = fields.boolean("hostileToCriminals", notes)?;
        let hostile_to_stolen_vehicles = fields.boolean("hostileToStolenVehicles", notes)?;
        let faction_override = fields.record_link_to("factionOverride", LEGACY_FACTION, notes)?;
        let faction_to_override = fields.record_link_to("factionToOverride", FACTION, notes)?;
        let source = UnitSource::read(
            fields,
            [
                ("tag", tag.state()),
                ("useReputationHostility", use_reputation_hostility.state()),
                ("usePersonalHostility", use_personal_hostility.state()),
                ("useFactionHostility", use_faction_hostility.state()),
                ("policesLawfulTrespass", polices_lawful_trespass.state()),
                ("hostileToCriminals", hostile_to_criminals.state()),
                (
                    "hostileToStolenVehicles",
                    hostile_to_stolen_vehicles.state(),
                ),
                ("factionOverride", faction_override.state()),
                ("factionToOverride", faction_to_override.state()),
            ],
        )?;
        Ok(Self {
            source,
            tag,
            use_reputation_hostility,
            use_personal_hostility,
            use_faction_hostility,
            polices_lawful_trespass,
            hostile_to_criminals,
            hostile_to_stolen_vehicles,
            faction_override,
            faction_to_override,
        })
    }
}
