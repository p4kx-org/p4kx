// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    /// What the Loadout cannot decide without the game's spawn, in the form of `issues` but no
    /// problem: nothing was kept from being read. Absent when there is none.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) notes: Vec<ErrorShape>,
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
