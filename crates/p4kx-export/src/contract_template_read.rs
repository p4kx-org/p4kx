// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, Guid, RecordTree, StructValue, TagTree};

use crate::CatalogError;
use crate::contract_hauling::HaulingSource;
use crate::contract_template::{
    ActiveSettings, AutoFinish, ContractTemplate, TemplateObjective, TemplateProperty,
};
use crate::guid_text::CatalogGuid;
use crate::mission_hauling::{objective_orders, property_orders};
use crate::mission_hauling_levels::HaulingLevels;
use crate::mission_index::title_key;
use crate::mission_layers::one;
use crate::record_fields::{boolean, class_name, guid_value, integer, linked, structs, text};
use crate::record_read::{KindRead, ReadError, read_kind};
use crate::selection::Kind;
use crate::spawn_layers::{Level, SpawnPools};
use crate::spawn_model::SpawnLevel;
use crate::spawn_read::Reader;
use crate::texts::Texts;

const PROPERTY_VALUE_PREFIX: &str = "MissionPropertyValue_";

/// The deepest nesting of `childMissionPhases` that is followed; the Records of LIVE nest five
/// levels.
const MAX_PHASE_DEPTH: usize = 32;

/// The contract templates of a DataForge and what was left out.
pub(crate) type ContractTemplates<'f> = KindRead<ContractTemplate<'f>>;

/// Reads the `ContractTemplate` Records `filter` matches, ordered by GUID; a development leftover
/// is left out and counted.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    texts: &Texts<'f>,
    tags: &TagTree,
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<ContractTemplates<'f>, CatalogError> {
    read_kind(
        forge,
        Kind::ContractTemplates,
        (filter, cancel),
        |tree| template(tree, texts, tags),
        |template| template.guid,
    )
}

fn template<'f>(
    tree: &RecordTree<'f>,
    texts: &Texts<'f>,
    tags: &TagTree,
) -> Result<ContractTemplate<'f>, ReadError> {
    let root = Some(tree.root());
    let display = one(root, "contractDisplayInfo");
    let class = one(root, "contractClass");
    let reader = Reader::new(tags, tree.root());
    let spawns = Level::read(
        SpawnLevel::Template,
        &reader,
        (root, "contractProperties"),
        (None, ""),
        &mut SpawnPools::default(),
    )?
    .resolved();
    let hauling = property_orders(root, "contractProperties");
    let objective = objective_orders(root);
    let hauling_orders = HaulingLevels {
        properties: vec![(HaulingSource::Template, hauling.as_slice())],
        template_objective: &objective,
        broker_objective: &[],
    }
    .orders();
    Ok(ContractTemplate {
        guid: CatalogGuid(tree.guid()),
        class_name: class_name(tree),
        record_path: tree.record_path(),
        not_for_release: boolean(root, "notForRelease"),
        work_in_progress: boolean(root, "workInProgress"),
        title: texts.localised(title_key(display)),
        mission_type_guid: linked(display, "type"),
        illegal: boolean(display, "illegal"),
        show_life_time_in_mobi_glas: boolean(display, "showLifeTimeInMobiGlas"),
        pre_show_objectives: boolean(display, "preShowObjectives"),
        class_type: class.map(|class| class.type_name().to_owned()),
        settings: one(class, "additionalParams").map(settings),
        auto_finish: one(class, "autoFinishSettings").map(auto_finish),
        contract_properties: properties(root),
        ship_spawns: spawns.ship_spawns,
        location_queries: spawns.location_queries,
        hauling_orders,
        objectives: objectives(&reader, root),
    })
}

fn settings(settings: &StructValue<'_>) -> ActiveSettings {
    let flag = |name: &str| boolean(Some(settings), name);
    ActiveSettings {
        can_be_shared: flag("canBeShared"),
        has_complete_button: flag("hasCompleteButton"),
        handles_abandon_request: flag("handlesAbandonRequest"),
        display_allied_markers: flag("displayAlliedMarkers"),
        only_owner_can_complete: flag("onlyOwnerCanComplete"),
    }
}

fn auto_finish(settings: &StructValue<'_>) -> AutoFinish {
    let flag = |name: &str| boolean(Some(settings), name);
    let deadline = one(Some(settings), "contractDeadline");
    AutoFinish {
        fail_if_sent_to_prison: flag("failIfSentToPrison"),
        fail_if_became_criminal: flag("failIfBecameCriminal"),
        fail_if_leave_prison: flag("failIfLeavePrison"),
        completion_time: integer(deadline, "missionCompletionTime"),
        auto_end: boolean(deadline, "missionAutoEnd"),
        result_after_timer_end: text(deadline, "missionResultAfterTimerEnd").map(str::to_owned),
        remaining_time_to_show_timer: integer(deadline, "remainingTimeToShowTimer"),
    }
}

fn properties(root: Option<&StructValue<'_>>) -> Vec<TemplateProperty> {
    let owned = |property: &StructValue<'_>, name: &str| {
        text(Some(property), name)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    structs(root, "contractProperties")
        .map(|property| TemplateProperty {
            variable: owned(property, "missionVariableName"),
            token: owned(property, "extendedTextToken"),
            value_type: one(Some(property), "value").map(|value| {
                let name = value.type_name();
                name.strip_prefix(PROPERTY_VALUE_PREFIX)
                    .unwrap_or(name)
                    .to_owned()
            }),
        })
        .collect()
}

/// Reads the `objectiveTokens` and the phases below them, each token before its phases; a token
/// without an `id` and the phases below it are left out.
fn objectives(reader: &Reader<'_, '_>, root: Option<&StructValue<'_>>) -> Vec<TemplateObjective> {
    let mut found = Vec::new();
    let mut pending: Vec<(&StructValue<'_>, Option<CatalogGuid>, usize)> =
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
        found.push(TemplateObjective {
            id,
            parent_id,
            debug_name: text(Some(token), "debugName")
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
            starts_active: boolean(Some(token), "startsActive"),
            handler_type: one(Some(token), "objectiveHandler")
                .map(|handler| handler.type_name().to_owned()),
            phase_tag: reader.tag_path(Some(token), "missionPhaseIdentifierTag"),
        });
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
