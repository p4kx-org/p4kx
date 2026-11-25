// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Owner-local source value arenas for physical Loot Records.

use crate::Kind;

#[path = "loot_definition.rs"]
mod definition;
#[path = "loot_definition_read.rs"]
mod read;

#[path = "loot_definition_kind.rs"]
mod kind;
#[path = "loot_definition_tables.rs"]
mod tables;

pub use definition::LootDefinition;
pub use kind::LootDefinitionKind;
pub(crate) use read::{read, read_archetype, read_multi_config, read_projected, read_table};
pub(crate) use tables::{LootDefinitionTables, LootDefinitions};

pub(crate) const fn has_authored_arena(kind: Kind) -> bool {
    LootDefinitionKind::of(kind).is_some()
        || matches!(kind, Kind::LegacyLootTables | Kind::LegacyLootArchetypes)
}
