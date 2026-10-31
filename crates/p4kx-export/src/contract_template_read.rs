// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree, StructValue, TagTree};

use crate::CatalogError;
use crate::contract_hauling::HaulingSource;
use crate::contract_template::{ActiveSettings, AutoFinish, ContractTemplate, TemplateProperty};
use crate::guid_text::CatalogGuid;
use crate::mission_hauling::{objective_orders, property_orders};
use crate::mission_hauling_levels::HaulingLevels;
use crate::mission_index::title_key;
use crate::mission_layers::one;
use crate::record_fields::{boolean, class_name, integer, linked, structs, text};
use crate::record_read::{KindRead, ReadError, read_kind};
use crate::selection::Kind;
use crate::spawn_layers::{Level, SpawnPools};
use crate::spawn_model::SpawnLevel;
use crate::spawn_read::Reader;
use crate::template_extras::{end_comms, modifiers, owner, partial_payout, start_comms};
use crate::template_flow::triggers;
use crate::template_objective::ObjectiveState;
use crate::template_objective_read::ObjectiveReader;
use crate::texts::Texts;

const PROPERTY_VALUE_PREFIX: &str = "MissionPropertyValue_";

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
        |tree| template(forge, tree, texts, tags),
        |template| template.guid,
    )
}

fn template<'f>(
    forge: &DataForge<'f>,
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
        objectives: ObjectiveReader {
            reader: &reader,
            texts,
        }
        .objectives(root),
        partial_payout: partial_payout(root),
        modifiers: modifiers(&reader, root),
        owner: owner(forge, root),
        mission_flow: triggers(&reader, root),
        start_comms: start_comms(&reader, root),
        end_comms: end_comms(&reader, root),
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
        result_after_timer_end: text(deadline, "missionResultAfterTimerEnd")
            .and_then(ObjectiveState::parse),
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
