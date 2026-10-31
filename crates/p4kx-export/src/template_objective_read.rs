// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, StructValue, Value};

use crate::contract::LocalisedText;
use crate::guid_text::CatalogGuid;
use crate::mission_layers::{items, key, one};
use crate::record_fields::{boolean, guid_value, integer, linked, number, structs, text};
use crate::spawn_read::Reader;
use crate::template_objective::{
    CommsTiming, HaulingSettings, InstanceEntry, NavPoint, ObjectiveCategory, ObjectiveComms,
    ObjectiveDisplay, ObjectiveHandler, ObjectiveState, OverrideDisplay, RewardContribution,
    TemplateObjective,
};
use crate::texts::Texts;

/// The deepest nesting of `childMissionPhases` that is followed; the Records of LIVE nest five
/// levels.
const MAX_PHASE_DEPTH: usize = 32;

/// What the objectives of one Record are read with: its tags and weak pointers, and the texts.
pub(crate) struct ObjectiveReader<'r, 'x, 'f> {
    pub(crate) reader: &'r Reader<'x, 'f>,
    pub(crate) texts: &'r Texts<'f>,
}

impl<'f> ObjectiveReader<'_, '_, 'f> {
    /// Reads the `objectiveTokens` of `root` and the phases below them, each token before its
    /// phases; a token without an `id` and the phases below it are left out.
    pub(crate) fn objectives(&self, root: Option<&StructValue<'f>>) -> Vec<TemplateObjective<'f>> {
        let mut found = Vec::new();
        let mut pending: Vec<(&StructValue<'f>, Option<CatalogGuid>, usize)> =
            structs(root, "objectiveTokens")
                .map(|token| (token, None, 0))
                .collect();
        pending.reverse();
        while let Some((token, parent_id, depth)) = pending.pop() {
            let Some(id) = guid_value(Some(token), "id")
                .filter(|id| *id != Guid::default())
                .map(CatalogGuid)
            else {
                continue;
            };
            found.push(self.objective(token, id, parent_id));
            if depth < MAX_PHASE_DEPTH {
                let mut phases: Vec<_> = structs(Some(token), "childMissionPhases")
                    .map(|phase| (phase, Some(id), depth.saturating_add(1)))
                    .collect();
                phases.reverse();
                pending.append(&mut phases);
            }
        }
        found
    }

    fn objective(
        &self,
        token: &StructValue<'f>,
        id: CatalogGuid,
        parent_id: Option<CatalogGuid>,
    ) -> TemplateObjective<'f> {
        let token = Some(token);
        let override_display = one(token, "overrideMissionDetailsDisplayInfo");
        TemplateObjective {
            id,
            parent_id,
            debug_name: owned(token, "debugName"),
            starts_active: boolean(token, "startsActive"),
            variable_name: owned(token, "variableName"),
            weight: number(token, "weight"),
            phase_tag: self.reader.tag_path(token, "missionPhaseIdentifierTag"),
            handler: one(token, "objectiveHandler").map(|handler| self.handler(handler)),
            display: self.display(one(token, "displayInfo")),
            override_display: OverrideDisplay {
                title_override: self.text(override_display, "titleOverride"),
                description_override: self.text(override_display, "descriptionOverride"),
            },
            reward_contribution: one(token, "rewardContribution").map(|contribution| {
                RewardContribution {
                    struct_name: contribution.type_name().to_owned(),
                    percent_contribution: integer(Some(contribution), "percentContribution"),
                    do_partial_reward_payout: boolean(Some(contribution), "doPartialRewardPayout"),
                }
            }),
            comms: structs(token, "commsNotifications")
                .map(|comms| self.comms(comms))
                .collect(),
        }
    }

    fn comms(&self, comms: &StructValue<'f>) -> ObjectiveComms {
        let comms = Some(comms);
        ObjectiveComms {
            trigger: text(comms, "trigger").and_then(ObjectiveState::parse),
            timing: text(comms, "commsTiming").and_then(CommsTiming::parse),
            delay: integer(comms, "delay"),
            communication_tag: self.reader.tag_path(comms, "communicationTags"),
        }
    }

    fn text(&self, parent: Option<&StructValue<'f>>, name: &str) -> LocalisedText<'f> {
        self.texts.localised(key(parent, name))
    }

    fn display(&self, display: Option<&StructValue<'f>>) -> ObjectiveDisplay<'f> {
        ObjectiveDisplay {
            category: text(display, "category").and_then(ObjectiveCategory::parse),
            hide_on_hud: boolean(display, "hideOnHUD"),
            marker_label: self.text(display, "objectiveMarkerLabel"),
            short_description: self.text(display, "shortDescription"),
            long_description: self.text(display, "longDescription"),
        }
    }

    fn optional_display(
        &self,
        parent: Option<&StructValue<'f>>,
        name: &str,
    ) -> Option<ObjectiveDisplay<'f>> {
        one(parent, name).map(|display| self.display(Some(display)))
    }

    fn handler(&self, handler: &StructValue<'f>) -> ObjectiveHandler<'f> {
        let parent = Some(handler);
        let variable = |name: &str| self.reader.property_variable(parent, name);
        ObjectiveHandler {
            struct_name: handler.type_name().to_owned(),
            module: owned(parent, "module"),
            module_hierarchy_guid: linked(parent, "moduleHierarchy"),
            disable_travel_objectives: boolean(parent, "disableTravelObjectives"),
            disable_return_objectives: boolean(parent, "disableReturnObjectives"),
            travel_radius_km: number(parent, "travelRadiusKM"),
            all_players_left_grace_period: number(parent, "allPlayersLeftGracePeriod"),
            location_property: variable("location"),
            organization_property: variable("organization"),
            hauling_limit_property: variable("haulingLimitProperty"),
            target_property: variable("targetObject"),
            min_distance: integer(parent, "minDistance"),
            max_distance: integer(parent, "maxDistance"),
            hide_markers: boolean(parent, "hideMarkers"),
            security_manifest_guid: linked(parent, "securityManifestToOverride"),
            nav_point: one(parent, "navPointSpawnInfo").map(|nav| self.nav_point(nav)),
            travel_objective: self.optional_display(parent, "travelObjectiveInfo"),
            return_objective: self.optional_display(parent, "returnObjectiveInfo"),
            hauling: one(parent, "objectiveSettings").map(|settings| self.hauling(settings)),
            instance: self.instance(handler),
        }
    }

    fn nav_point(&self, nav: &StructValue<'f>) -> NavPoint<'f> {
        let nav = Some(nav);
        NavPoint {
            entity_class_guid: linked(nav, "entityClassDefinition"),
            display_name: self.text(nav, "name"),
            extended_text_tokens: items(nav, "extendedTextTokens")
                .iter()
                .filter_map(|value| match value {
                    Value::String(token) if !token.is_empty() => Some((*token).to_owned()),
                    _ => None,
                })
                .collect(),
        }
    }

    fn hauling(&self, settings: &StructValue<'f>) -> HaulingSettings<'f> {
        let parent = Some(settings);
        HaulingSettings {
            item_token: owned(parent, "itemExtendedTextToken"),
            pickup_location_token: owned(parent, "pickupLocationExtendedTextToken"),
            amount_token: owned(parent, "amountExtendedTextToken"),
            total_token: owned(parent, "totalExtendedTextToken"),
            drop_off_location_token: owned(parent, "dropOffLocationExtendedTextToken"),
            and_objective: self.optional_display(parent, "andObjective"),
            or_objective: self.optional_display(parent, "orObjective"),
            collect_objective: self.optional_display(parent, "collectObjective"),
            collect_and_deliver_objective: self
                .optional_display(parent, "collectAndDeliverObjective"),
            resource_deliver_objective: self.optional_display(parent, "resourceDeliverObjective"),
            item_deliver_objective: self.optional_display(parent, "itemDeliverObjective"),
            list_item_objective: self.optional_display(parent, "listItemObjective"),
        }
    }

    /// Returns the fields of a handler that transits to an instance, `None` for any other.
    fn instance(&self, handler: &StructValue<'f>) -> Option<InstanceEntry<'f>> {
        let parent = Some(handler);
        handler.field("instanceTagIdentifier")?;
        Some(InstanceEntry {
            complete_phase_on_player_enter: boolean(parent, "completePhaseOnPlayerEnterInstance"),
            enter_travel_radius_km: number(parent, "instanceEnterTravelRadiusKM"),
            hud_objective_text: self.text(parent, "instanceHUDObjectiveText"),
            tag_identifier: self.reader.tag_path(parent, "instanceTagIdentifier"),
            search_tags: self.reader.tag_array(parent, "ocTagsToSearchInstanceEnter"),
            objective_marker: owned(parent, "transitToInstanceObjectiveMarker"),
        })
    }
}

fn owned(parent: Option<&StructValue<'_>>, name: &str) -> Option<String> {
    text(parent, name).map(str::to_owned)
}
