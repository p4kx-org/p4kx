// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Owner-local source value arenas for physical Loot Records.

#[path = "loot_definition.rs"]
mod definition;
#[path = "loot_definition_read.rs"]
mod read;

#[path = "loot_definition_kind.rs"]
mod kind;
#[path = "loot_definition_tables.rs"]
mod tables;

pub(crate) use definition::LootDefinition;
pub(crate) use kind::LootDefinitionKind;
pub(crate) use read::read;
pub(crate) use tables::{LootDefinitionTables, LootDefinitions};
