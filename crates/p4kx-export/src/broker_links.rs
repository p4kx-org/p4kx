// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The links of a mission broker entry to other Records and the tags it states.

use p4kx_extraction::StructValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::record_fields::{linked, linked_list, structs};
use crate::spawn_model::{TagPaths, TagTerm};
use crate::spawn_read::Reader;

/// What an entry links to; a GUID the Archive lacks is kept.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Links {
    /// The `type`, an entry of `missionTypes`.
    pub(crate) mission_type_guid: Option<CatalogGuid>,
    /// The `locationMissionAvailable`, an entry of `starmapObjects`.
    pub(crate) location_guid: Option<CatalogGuid>,
    /// The `localityAvailable`, an entry of `missionLocalities`.
    pub(crate) locality_guid: Option<CatalogGuid>,
    /// The `missionGiverRecord`, a mission giver Record; no kind lists these.
    pub(crate) giver_guid: Option<CatalogGuid>,
    /// The `invitationMission`, an entry of `missionBrokerEntries`.
    pub(crate) invitation_mission_guid: Option<CatalogGuid>,
    /// The `linkedMission`, an entry of `missionBrokerEntries`.
    pub(crate) linked_mission_guid: Option<CatalogGuid>,
    /// The `associatedMissions`, entries of `missionBrokerEntries`, in stored order.
    pub(crate) associated_mission_guids: Vec<CatalogGuid>,
    /// The `requiredMissions`, entries of `missionBrokerEntries`, in stored order.
    pub(crate) required_mission_guids: Vec<CatalogGuid>,
    /// The `onlyAvailableIfAllMissionsNotAvailable`, entries of `missionBrokerEntries`: the entry
    /// is offered only while none of them is.
    pub(crate) unless_available_mission_guids: Vec<CatalogGuid>,
    /// The `journalEntriesToAddOnComplete`, journal Records; no kind lists these.
    pub(crate) journal_entries_on_complete: Vec<CatalogGuid>,
    /// The `journalEntriesToRemoveOnComplete`, journal Records; no kind lists these.
    pub(crate) journal_entries_on_remove: Vec<CatalogGuid>,
}

/// The tags an entry grants and asks for, as tag paths.
#[expect(
    clippy::struct_field_names,
    reason = "every field is the stored name of a tag field"
)]
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Tags {
    /// The `completionTags` the entry grants.
    pub(crate) completion_tags: TagPaths,
    /// The `missionTags`.
    pub(crate) mission_tags: TagPaths,
    /// The `requiredAreaTags`.
    pub(crate) required_area_tags: TagPaths,
    /// The `excludedAreaTags`.
    pub(crate) excluded_area_tags: TagPaths,
    /// The `requiredCompletedMissionTags`: the player has completed missions that carry the
    /// positive and none that carry the negative tags of each term.
    pub(crate) required_completed_tags: Vec<TagTerm>,
}

/// Reads the links of `root`.
pub(crate) fn links(root: Option<&StructValue<'_>>) -> Links {
    Links {
        mission_type_guid: linked(root, "type"),
        location_guid: linked(root, "locationMissionAvailable"),
        locality_guid: linked(root, "localityAvailable"),
        giver_guid: linked(root, "missionGiverRecord"),
        invitation_mission_guid: linked(root, "invitationMission"),
        linked_mission_guid: linked(root, "linkedMission"),
        associated_mission_guids: linked_list(root, "associatedMissions"),
        required_mission_guids: linked_list(root, "requiredMissions"),
        unless_available_mission_guids: linked_list(root, "onlyAvailableIfAllMissionsNotAvailable"),
        journal_entries_on_complete: linked_list(root, "journalEntriesToAddOnComplete"),
        journal_entries_on_remove: linked_list(root, "journalEntriesToRemoveOnComplete"),
    }
}

/// Reads the tags of `root`.
pub(crate) fn tags(reader: &Reader<'_, '_>, root: Option<&StructValue<'_>>) -> Tags {
    Tags {
        completion_tags: reader.tag_list(root, "completionTags"),
        mission_tags: reader.tag_array(root, "missionTags"),
        required_area_tags: reader.tag_list(root, "requiredAreaTags"),
        excluded_area_tags: reader.tag_list(root, "excludedAreaTags"),
        required_completed_tags: structs(root, "requiredCompletedMissionTags")
            .map(|term| TagTerm {
                positive_tags: reader.tag_array(Some(term), "positiveTags"),
                negative_tags: reader.tag_array(Some(term), "negativeTags"),
            })
            .collect(),
    }
}
