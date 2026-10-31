// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, Attachment, EntityNode, LoadoutChanges, LoadoutError, LoadoutPicks,
    LoadoutResolver, PortDefinition, PortNode, PortVisibility, Target,
};
use p4kx_extraction::DataForge;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::archives::Opened;
use crate::budget::{fill_within, json_len};
use crate::error::{ToolError, capped};
use crate::page::Page;

/// The deepest port nesting a call returns when it gives no depth.
pub(crate) const DEFAULT_DEPTH: u32 = 16;
/// The deepest port nesting a call may ask for.
pub(crate) const MAX_DEPTH: u32 = 32;
/// The most characters of a text.
const MAX_TEXT_CHARS: usize = 256;
/// The most characters of a port path.
const MAX_PATH_CHARS: usize = 1024;
/// The most picks a call may list.
const MAX_PICKS: usize = 64;
/// The most issues listed per Entity.
const MAX_ISSUES: usize = 64;
/// The most item types listed per port.
const MAX_TYPES: usize = 16;
/// The most sub-types listed per item type.
const MAX_SUB_TYPES: usize = 16;

/// The arguments of `entity_loadout`.
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoadoutParams {
    /// A Channel name such as `LIVE`, or the absolute path of a `.p4k` file; omit it for the
    /// Archive the server was started with.
    pub(crate) archive: Option<String>,
    /// The Entity: a class name such as `AEGS_Avenger_Titan`, a record name, a record path or a
    /// GUID.
    pub(crate) entity: String,
    /// The deepest port nesting returned, 16 by default, at most 32; the top level ports are 1.
    pub(crate) max_depth: Option<u32>,
    /// Leave the Loadouts of docked vehicles unresolved; they are listed as ports.
    pub(crate) stop_at_docked_vehicles: Option<bool>,
    /// Alternatives of the groups of generated default Loadouts (creatures, NPCs) to take instead
    /// of the first, each `<group>=<index>` such as `Body=2`, or a bare `<index>` for every group
    /// that has it; indices count from 0. Several picks may share a text, separated by commas.
    /// An index beyond a named group, and a named group the Loadout lacks, is an issue of the
    /// Entity and the group keeps its first alternative.
    pub(crate) loadout_pick: Option<Vec<String>>,
    /// The `next_cursor` of the previous page; omit it for the first page.
    pub(crate) cursor: Option<String>,
    /// The page size in ports, 100 by default, at most 1000.
    pub(crate) limit: Option<u32>,
}

/// The Loadout tree of an Entity as a list of ports, parents before their children.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct LoadoutResult {
    /// The path of the Archive.
    pub(crate) archive: String,
    /// The Entity.
    pub(crate) entity: EntityShape,
    /// The ports of the whole tree by what they hold.
    pub(crate) counts: Counts,
    /// The number of ports at or above `max_depth`.
    pub(crate) total: usize,
    /// The number of ports left out because they lie deeper than `max_depth`.
    pub(crate) deeper: usize,
    /// The ports on this page, depth first in port order.
    pub(crate) ports: Vec<PortShape>,
    /// The cursor of the next page; absent on the last page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) next_cursor: Option<String>,
}

/// An Entity.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct EntityShape {
    /// The GUID of the Entity's Record.
    pub(crate) guid: String,
    /// The record path.
    pub(crate) record_path: String,
    /// The Record name.
    pub(crate) name: String,
    /// The class name.
    pub(crate) class_name: String,
    /// Whether it is a vehicle docked in a port of another.
    pub(crate) docked_vehicle: bool,
    /// Whether the Loadout of a docked vehicle is resolved.
    pub(crate) expanded: bool,
    /// Why the Entity's own Loadout cannot be read in full, at most 64.
    pub(crate) issues: Vec<ErrorShape>,
    /// The number of such reasons.
    pub(crate) issue_count: usize,
}

/// The ports of a tree by what they hold.
#[derive(Debug, Default, Serialize, JsonSchema)]
pub(crate) struct Counts {
    /// All ports.
    pub(crate) ports: usize,
    /// Ports with an Entity attached.
    pub(crate) attached: usize,
    /// Ports that hold nothing.
    pub(crate) empty: usize,
    /// Ports whose attachment cannot be resolved.
    pub(crate) unresolved: usize,
}

/// One port.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct PortShape {
    /// The port names from the root joined by `/`.
    pub(crate) path: String,
    /// The nesting: 1 for a port of the root Entity.
    pub(crate) depth: usize,
    /// The port name.
    pub(crate) name: String,
    /// Where the attachment comes from: `loadout`, `override`, `port_default`, `unset`,
    /// `empty` (a port only the vehicle implementation declares), `excluded` or `replaced`; new
    /// values may be added.
    pub(crate) source: &'static str,
    /// `visible` or `invisible` when the vehicle implementation or the port definition's flags
    /// state it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) visibility: Option<&'static str>,
    /// What the port takes, when the Entity Record defines it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) definition: Option<DefinitionShape>,
    /// The Entity attached; absent for an empty port and an unresolved one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) entity: Option<EntityShape>,
    /// Why the attachment cannot be resolved.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) unresolved: Option<ErrorShape>,
}

/// What a port takes.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct DefinitionShape {
    /// The port's display name as the Record gives it.
    pub(crate) display_name: String,
    /// The port flags.
    pub(crate) flags: String,
    /// The smallest item size.
    pub(crate) min_size: Option<u32>,
    /// The largest item size.
    pub(crate) max_size: Option<u32>,
    /// The item types taken, at most 16.
    pub(crate) types: Vec<TypeShape>,
    /// The number of item types taken.
    pub(crate) type_count: usize,
    /// The helper of the parent's model the port attaches to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) helper: Option<String>,
}

/// An item type a port takes.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct TypeShape {
    /// The type, e.g. `WeaponGun`.
    pub(crate) name: String,
    /// The sub-types, at most 16; empty when every sub-type is taken.
    pub(crate) sub_types: Vec<String>,
    /// The number of sub-types.
    pub(crate) sub_type_count: usize,
}

/// A reason a port or an Entity cannot be resolved.
#[derive(Debug, Serialize, JsonSchema)]
pub(crate) struct ErrorShape {
    /// A stable code such as `dangling_reference` or `unknown_class`.
    pub(crate) kind: &'static str,
    /// The reason as text.
    pub(crate) reason: String,
    /// The GUID the Loadout names, if it names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reference: Option<String>,
    /// The class name the Loadout names, if it names one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) class_name: Option<String>,
}

/// Resolves the Loadout of the Entity `params` names and returns one page of its ports.
pub(crate) fn resolve(
    opened: &Opened,
    forge: &DataForge<'_>,
    params: &LoadoutParams,
    page: Page,
) -> Result<LoadoutResult, ToolError> {
    let stop_at_docked = params.stop_at_docked_vehicles == Some(true);
    let picks = picks_of(params.loadout_pick.as_deref().unwrap_or_default())?;
    let key = picks.normalised();
    let tree = opened
        .loadouts()
        .get_or_resolve(&params.entity, (stop_at_docked, &key), || {
            let files = ArchiveFiles::new(opened.archive());
            let mut resolver = LoadoutResolver::new(forge).with_files(&files);
            if stop_at_docked {
                resolver = resolver.stopping_at_docked_vehicles();
            }
            let record = resolver.entities().find(&params.entity)?;
            let changes = LoadoutChanges::new().with_picks(picks.clone());
            Ok(resolver.resolve_with(&record, &changes)?)
        })?;
    let max_depth = usize::try_from(
        params
            .max_depth
            .unwrap_or(DEFAULT_DEPTH)
            .clamp(1, MAX_DEPTH),
    )
    .unwrap_or(1);
    let all = tree.walk();
    let counts = counts(&all);
    let near: Vec<(&String, &PortNode)> = all
        .iter()
        .filter(|(path, _)| depth(path) <= max_depth)
        .map(|(path, port)| (path, *port))
        .collect();
    let mut result = LoadoutResult {
        archive: opened.archive().path().display().to_string(),
        entity: entity_shape(&tree),
        counts,
        total: near.len(),
        deeper: all.len() - near.len(),
        ports: Vec::new(),
        next_cursor: None,
    };
    let items = page
        .slice(&near)
        .iter()
        .map(|&(path, port)| Ok(port_shape(path, port)));
    let (ports, next_cursor) = fill_within(
        page,
        near.len(),
        json_len(&result),
        items,
        |shape: &PortShape| format!("port {}", shape.path),
    )?;
    result.ports = ports;
    result.next_cursor = next_cursor;
    Ok(result)
}

/// Reads the picks of a call: each text `<group>=<index>` or a bare `<index>`, several to a text
/// separated by commas.
pub(crate) fn picks_of(texts: &[String]) -> Result<LoadoutPicks, ToolError> {
    if texts.len() > MAX_PICKS {
        return Err(ToolError::Refused(format!(
            "loadout_pick lists {} picks; at most {MAX_PICKS} are accepted",
            texts.len()
        )));
    }
    for text in texts {
        capped("loadout_pick", text, MAX_TEXT_CHARS)?;
    }
    Ok(LoadoutPicks::from_list(&texts.join(","))?)
}

fn depth(path: &str) -> usize {
    path.matches('/').count() + 1
}

fn counts(all: &[(String, &PortNode)]) -> Counts {
    let mut counts = Counts {
        ports: all.len(),
        ..Counts::default()
    };
    for (_, port) in all {
        match port.attachment() {
            Attachment::Empty => counts.empty += 1,
            Attachment::Entity(_) => counts.attached += 1,
            Attachment::Unresolved(_) => counts.unresolved += 1,
        }
    }
    counts
}

fn entity_shape(node: &EntityNode) -> EntityShape {
    EntityShape {
        guid: node.guid().to_string(),
        record_path: clip(node.record_path()),
        name: clip(node.name()),
        class_name: clip(node.class_name()),
        docked_vehicle: node.is_docked_vehicle(),
        expanded: node.is_expanded(),
        issues: node
            .issues()
            .iter()
            .take(MAX_ISSUES)
            .map(|error| error_shape(error, None))
            .collect(),
        issue_count: node.issues().len(),
    }
}

fn port_shape(path: &str, port: &PortNode) -> PortShape {
    let (entity, unresolved) = match port.attachment() {
        Attachment::Empty => (None, None),
        Attachment::Entity(node) => (Some(entity_shape(node)), None),
        Attachment::Unresolved(failed) => (
            None,
            Some(error_shape(failed.error(), Some(failed.target()))),
        ),
    };
    PortShape {
        path: path.chars().take(MAX_PATH_CHARS).collect(),
        depth: depth(path),
        name: clip(port.name()),
        source: port.source().code(),
        visibility: match port.visibility() {
            PortVisibility::Unspecified => None,
            stated => Some(stated.code()),
        },
        definition: port.definition().map(definition_shape),
        entity,
        unresolved,
    }
}

fn definition_shape(definition: &PortDefinition) -> DefinitionShape {
    DefinitionShape {
        display_name: clip(definition.display_name()),
        flags: clip(definition.flags()),
        min_size: definition.min_size(),
        max_size: definition.max_size(),
        types: definition
            .types()
            .iter()
            .take(MAX_TYPES)
            .map(|item| TypeShape {
                name: clip(item.name()),
                sub_types: item
                    .sub_types()
                    .iter()
                    .take(MAX_SUB_TYPES)
                    .map(|text| clip(text))
                    .collect(),
                sub_type_count: item.sub_types().len(),
            })
            .collect(),
        type_count: definition.types().len(),
        helper: definition
            .transform()
            .map(|transform| clip(transform.helper()))
            .filter(|helper| !helper.is_empty()),
    }
}

fn error_shape(error: &LoadoutError, target: Option<&Target>) -> ErrorShape {
    let (reference, class_name) = match target {
        Some(Target::Reference(guid)) => (Some(guid.to_string()), None),
        Some(Target::ClassName(name)) => (None, Some(clip(name))),
        _ => (None, None),
    };
    ErrorShape {
        kind: error.kind(),
        reason: clip(&error.to_string()),
        reference,
        class_name,
    }
}

fn clip(text: &str) -> String {
    text.chars().take(MAX_TEXT_CHARS).collect()
}
