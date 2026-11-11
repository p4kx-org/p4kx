// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Complete original Loot filter Records in one owner-local value arena.

#[path = "loot_filter_definition.rs"]
mod definition;
#[path = "loot_filter_read.rs"]
mod read;

pub(crate) use definition::LootFilterDefinition;
pub(crate) use read::read;
