// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Filter, RecordTree, StructValue, TagTree};

use crate::CatalogError;
use crate::contract_hauling::{HaulingOrder, HaulingSource};
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
use crate::template_extras::{end_comms, owner, partial_payout, start_comms};
use crate::template_flow::FlowReader;
use crate::template_issues::Unknowns;
use crate::template_modifier::modifiers;
use crate::template_objective::ObjectiveState;
use crate::template_objective_read::ObjectiveReader;
use crate::texts::Texts;

const PROPERTY_VALUE_PREFIX: &str = "MissionPropertyValue_";

/// The contract templates of a DataForge, what was left out, the ship spawns and location queries
/// their objectives state and the stored names the catalog does not know.
pub(crate) struct ContractTemplates<'f> {
    pub(crate) read: KindRead<ContractTemplate<'f>>,
    pub(crate) pools: SpawnPools,
    pub(crate) unknowns: Unknowns,
}

/// What a template is read with.
struct Sources<'r, 'f> {
    forge: &'r DataForge<'f>,
    texts: &'r Texts<'f>,
    tags: &'r TagTree,
    unknowns: &'r Unknowns,
}

/// Reads the `ContractTemplate` Records `filter` matches, ordered by GUID; a development leftover
/// is left out and counted. A Record that nests its phases or conditions too deep is a failure.
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
    let unknowns = Unknowns::default();
    let sources = Sources {
        forge,
        texts,
        tags,
        unknowns: &unknowns,
    };
    let read = read_kind(
        forge,
        Kind::ContractTemplates,
        (filter, cancel),
        |tree| template(&sources, tree),
        |(template, _)| template.guid,
    )?;
    let mut pools = SpawnPools::default();
    let items = read
        .items
        .into_iter()
        .map(|(template, found)| {
            pools.merge(found);
            template
        })
        .collect();
    Ok(ContractTemplates {
        read: KindRead {
            items,
            skipped: read.skipped,
            failures: read.failures,
        },
        pools,
        unknowns,
    })
}

fn template<'f>(
    sources: &Sources<'_, 'f>,
    tree: &RecordTree<'f>,
) -> Result<(ContractTemplate<'f>, SpawnPools), ReadError> {
    let Sources {
        forge,
        texts,
        tags,
        unknowns,
    } = *sources;
    let root = Some(tree.root());
    let display = one(root, "contractDisplayInfo");
    let class = one(root, "contractClass");
    let reader = Reader::new(tags, tree.root());
    let pools = RefCell::new(SpawnPools::default());
    let spawns = Level::read(
        SpawnLevel::Template,
        &reader,
        (root, "contractProperties"),
        (None, ""),
        &mut pools.borrow_mut(),
    )?
    .resolved();
    let objectives = ObjectiveReader::new(&reader, texts, unknowns, &pools).objectives(root)?;
    let flow = FlowReader {
        reader: &reader,
        unknowns,
    };
    let template = ContractTemplate {
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
        auto_finish: one(class, "autoFinishSettings").map(|finish| auto_finish(finish, unknowns)),
        contract_properties: properties(root, "contractProperties"),
        ship_spawns: spawns.ship_spawns,
        location_queries: spawns.location_queries,
        hauling_orders: hauling_orders(root),
        objectives,
        partial_payout: partial_payout(root),
        modifiers: modifiers(&reader, unknowns, root)?,
        owner: owner(forge, root),
        mission_flow: flow.triggers(root)?,
        start_comms: start_comms(&reader, root),
        end_comms: end_comms(&reader, unknowns, root),
    };
    Ok((template, pools.into_inner()))
}

/// Returns the hauling orders of the template `root`: its hauling properties and the orders of its
/// hauling objectives.
fn hauling_orders<'f>(root: Option<&StructValue<'f>>) -> Vec<HaulingOrder<'f>> {
    let hauling = property_orders(root, "contractProperties");
    let objective = objective_orders(root);
    HaulingLevels {
        properties: vec![(HaulingSource::Template, hauling.as_slice())],
        template_objective: &objective,
        broker_objective: &[],
    }
    .orders()
}

/// Reads the `ActiveContractSettings` flags of the struct `settings`.
pub(crate) fn settings(settings: &StructValue<'_>) -> ActiveSettings {
    let flag = |name: &str| boolean(Some(settings), name);
    ActiveSettings {
        can_be_shared: flag("canBeShared"),
        has_complete_button: flag("hasCompleteButton"),
        handles_abandon_request: flag("handlesAbandonRequest"),
        display_allied_markers: flag("displayAlliedMarkers"),
        only_owner_can_complete: flag("onlyOwnerCanComplete"),
    }
}

fn auto_finish(settings: &StructValue<'_>, unknowns: &Unknowns) -> AutoFinish {
    let flag = |name: &str| boolean(Some(settings), name);
    let deadline = one(Some(settings), "contractDeadline");
    AutoFinish {
        fail_if_sent_to_prison: flag("failIfSentToPrison"),
        fail_if_became_criminal: flag("failIfBecameCriminal"),
        fail_if_leave_prison: flag("failIfLeavePrison"),
        completion_time: integer(deadline, "missionCompletionTime"),
        auto_end: boolean(deadline, "missionAutoEnd"),
        result_after_timer_end: unknowns.parse(
            "timer end result",
            text(deadline, "missionResultAfterTimerEnd"),
            ObjectiveState::parse,
        ),
        remaining_time_to_show_timer: integer(deadline, "remainingTimeToShowTimer"),
    }
}

/// Reads the mission properties the array field `field` of `root` holds: the variable, the token
/// and the kind of value of each.
pub(crate) fn properties(root: Option<&StructValue<'_>>, field: &str) -> Vec<TemplateProperty> {
    let owned = |property: &StructValue<'_>, name: &str| {
        text(Some(property), name)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    structs(root, field)
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
