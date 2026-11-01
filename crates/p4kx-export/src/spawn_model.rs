// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_extraction::Guid;
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest as _, Sha256};

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;

/// The level of a contract that states a property or a prerequisite.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum SpawnLevel {
    /// The overrides of the sub contract.
    SubContract,
    /// The overrides of the contract.
    Contract,
    /// The `contractParams` of the handler.
    Handler,
    /// The `contractProperties` of the `ContractTemplate`.
    Template,
    /// The `properties` of the `MissionBrokerEntry`.
    Broker,
}

/// A set of tags written as their full tag paths, e.g. `AI.Ship.SpawnFlags.PoweredOff`, in
/// stored order.
pub(crate) type TagPaths = Vec<String>;

/// An [`io::Write`] that feeds what it is given to a SHA-256 hasher.
struct HashWriter(Sha256);

impl io::Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Returns the identifier of a pool: the first 16 bytes of the SHA-256 of its canonical JSON.
///
/// The Archive gives these inline structures no GUID, so equal content shares one entry and one
/// identifier; the identifier holds as long as the content does.
///
/// # Errors
///
/// Returns the error of the JSON serialisation of `content`.
pub(crate) fn content_guid(content: &impl Serialize) -> Result<CatalogGuid, serde_json::Error> {
    let mut hasher = HashWriter(Sha256::new());
    serde_json::to_writer(&mut hasher, content)?;
    let mut bytes = [0_u8; 16];
    for (target, source) in bytes.iter_mut().zip(hasher.0.finalize()) {
        *target = source;
    }
    Ok(CatalogGuid(Guid::from_stored(bytes)))
}

/// One alternative of a ship slot: a ship the game picks by tags.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipSpawnOption {
    /// The `weight` of the alternative among its slot; `null` when absent.
    pub(crate) weight: Option<f32>,
    /// The `concurrentAmount`: how many of the ship are alive at once; `null` when absent.
    pub(crate) concurrent_amount: Option<i64>,
    /// The `includeLocationAISpawnTags` flag; `null` when absent.
    #[serde(rename = "includeLocationAISpawnTags")]
    pub(crate) include_location_ai_spawn_tags: Option<bool>,
    /// The `tags` the game searches ships and crew by: ship model, role, faction, cargo, spawn
    /// flags.
    pub(crate) tags: TagPaths,
    /// The `negativeTags` a match must not carry.
    pub(crate) negative_tags: TagPaths,
    /// The `markupTags`.
    pub(crate) markup_tags: TagPaths,
    /// The `entityTags`.
    pub(crate) entity_tags: TagPaths,
    /// The `initialDamageSettings` Record, `null` when absent.
    pub(crate) initial_damage_guid: Option<CatalogGuid>,
    /// The `missionVariableName` of the property the `nameProperty` points to, `null` when absent.
    pub(crate) name_property_variable: Option<String>,
}

/// The alternatives for one ship (`SpawnDescription_ShipOptions`): the game spawns one of them.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipSpawnSlot {
    /// The alternatives in stored order.
    pub(crate) options: Vec<ShipSpawnOption>,
}

/// A group of ships that spawn together (`SpawnDescription_ShipGroup`).
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipSpawnGroup {
    /// The `Name` of the group, `null` when empty.
    pub(crate) name: Option<String>,
    /// The ships of the group, one slot each.
    pub(crate) slots: Vec<ShipSpawnSlot>,
}

/// A ship spawn description of a contract: what a `MissionPropertyValue_ShipSpawnDescriptions`
/// property states, shared by every contract that states the same.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShipSpawnPool {
    /// The identifier of the content (see the catalog documentation); not a GUID of the Archive.
    pub(crate) guid: CatalogGuid,
    /// The `allowedForMissionRestrictedDeliveries` flag; `null` when absent.
    pub(crate) allowed_for_mission_restricted_deliveries: Option<bool>,
    /// The groups in stored order.
    pub(crate) groups: Vec<ShipSpawnGroup>,
}

/// The positive and negative tags of a tag search term.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct TagTerm {
    /// The tags a match carries.
    pub(crate) positive_tags: TagPaths,
    /// The tags a match must not carry.
    pub(crate) negative_tags: TagPaths,
}

/// One match condition of a location query (`DataSetMatchCondition_*`), as stored.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(tag = "condition", rename_all = "camelCase")]
pub(crate) enum LocationCondition {
    /// `DataSetMatchCondition_TagSearch`: the location must satisfy one of the terms.
    #[serde(rename_all = "camelCase")]
    TagSearch {
        /// The `tagType` (`General`, `Produces`, `Consumes`), `null` when absent.
        tag_type: Option<String>,
        /// The `tagSearch` terms.
        terms: Vec<TagTerm>,
    },
    /// `DataSetMatchCondition_ExcludeNearbyLocationsDef`.
    #[serde(rename_all = "camelCase")]
    ExcludeNearbyLocations {
        /// The `exclusionRangeKm`, `null` when absent.
        range_km: Option<i64>,
        /// The `missionVariableName` of the `locationProperty`, `null` when absent.
        location_property: Option<String>,
    },
    /// `DataSetMatchCondition_ExcludeDistantLocationsDef`.
    #[serde(rename_all = "camelCase")]
    ExcludeDistantLocations {
        /// The `exclusionRangeKm`, `null` when absent.
        range_km: Option<i64>,
        /// The `missionVariableName` of the `locationProperty`, `null` when absent.
        location_property: Option<String>,
    },
    /// `DataSetMatchCondition_ExcludeProperty`: the location differs from another property's.
    #[serde(rename_all = "camelCase")]
    ExcludeProperty {
        /// The `missionVariableName` of the `excludedProperty`, `null` when absent.
        excluded_property: Option<String>,
    },
    /// `DataSetMatchCondition_PropertyTags`: the tags of another property's location.
    #[serde(rename_all = "camelCase")]
    PropertyTags {
        /// The `propertyTagType`, `null` when absent.
        property_tag_type: Option<String>,
        /// The `targetTagType`, `null` when absent.
        target_tag_type: Option<String>,
        /// The `searchForAnyTag` flag, `null` when absent.
        search_for_any_tag: Option<bool>,
        /// The `missionVariableName` of the `searchProperty`, `null` when absent.
        search_property: Option<String>,
        /// The `tagFilters`.
        tag_filters: TagPaths,
    },
    /// `DataSetMatchCondition_SpecificOrganizationsDef`.
    #[serde(rename_all = "camelCase")]
    SpecificOrganizations {
        /// The `MissionOrganization` Records.
        organization_guids: Vec<CatalogGuid>,
    },
    /// `DataSetMatchCondition_SpecificItemsDef`.
    #[serde(rename_all = "camelCase")]
    SpecificItems {
        /// The `MissionItem` Records.
        item_guids: Vec<CatalogGuid>,
    },
    /// A condition of another struct, which the catalog does not read.
    #[serde(rename_all = "camelCase")]
    Other {
        /// The struct name.
        struct_name: String,
    },
}

/// Whether a location property holds one location or several.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = crate::closed_set::closed_string_set)]
pub(crate) enum LocationCount {
    /// `MissionPropertyValue_Location`.
    One,
    /// `MissionPropertyValue_Locations`.
    Many,
}

/// A location query of a contract: the tag search the game runs against the
/// `MissionLocationTemplate` Records when the mission starts. Shared by every contract that
/// states the same.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationQuery {
    /// The identifier of the content (see the catalog documentation); not a GUID of the Archive.
    pub(crate) guid: CatalogGuid,
    /// One location or several.
    pub(crate) count: LocationCount,
    /// The `minLocationsToFind` of a multiple query, `null` for a single one.
    pub(crate) min_locations: Option<i64>,
    /// The `maxLocationsToFind` of a multiple query, `null` for a single one.
    pub(crate) max_locations: Option<i64>,
    /// The `failIfMinAmountNotFound` flag of a multiple query, `null` for a single one.
    pub(crate) fail_if_min_not_found: Option<bool>,
    /// The `logErrorOnSearchFail` flag, `null` when absent.
    pub(crate) log_error_on_search_fail: Option<bool>,
    /// The `resourceTags`: the mission type tags the search is made for.
    pub(crate) resource_tags: TagPaths,
    /// The `matchConditions` in stored order; a location satisfies all of them.
    pub(crate) conditions: Vec<LocationCondition>,
}

/// The ship, NPC or entity spawn description a contract states for a variable.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct SpawnPoolLink {
    /// The `missionVariableName` of the property.
    pub(crate) variable: String,
    /// The `extendedTextToken` of the property, `null` when empty.
    pub(crate) token: Option<String>,
    /// The level that states the property: the first of sub contract, contract, handler and
    /// template.
    pub(crate) level: SpawnLevel,
    /// The `guid` of the entry of `contractShipSpawns`, `contractNpcSpawns` or
    /// `contractEntitySpawns`, by the field the link is in.
    pub(crate) pool_guid: CatalogGuid,
}

/// The location query a contract states for a variable.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationQueryLink {
    /// The `missionVariableName` of the property.
    pub(crate) variable: String,
    /// The `extendedTextToken` of the property, `null` when empty.
    pub(crate) token: Option<String>,
    /// The level that states the property: the first of sub contract, contract, handler and
    /// template.
    pub(crate) level: SpawnLevel,
    /// The `guid` of the entry of `contractLocationQueries`.
    pub(crate) query_guid: CatalogGuid,
}

/// A `ContractPrerequisite_LocationProperty`: the contract is offered at the location another
/// property resolves to.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct LocationPrerequisite {
    /// The level that states the prerequisite: `handler`, `contract` or `subContract`.
    pub(crate) level: SpawnLevel,
    /// The `propertyVariableName`, the `variable` of an entry of `locationQueries`.
    pub(crate) variable: String,
    /// The `propertyExtendedTextToken`, `null` when empty.
    pub(crate) token: Option<String>,
    /// The `locationLevelType` as stored (an enum name), `null` when absent.
    pub(crate) location_level_type: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identifier_is_the_start_of_the_sha256_of_the_canonical_json()
    -> Result<(), serde_json::Error> {
        let expected = Guid::from_stored([
            0x13, 0x5f, 0x17, 0xa4, 0x75, 0xa6, 0x1a, 0xfd, 0xee, 0xaf, 0x37, 0x59, 0xad, 0x2e,
            0x45, 0xad,
        ]);
        assert_eq!(content_guid(&("a", 1))?.0, expected, "SHA-256 of [\"a\",1]");
        Ok(())
    }
}
