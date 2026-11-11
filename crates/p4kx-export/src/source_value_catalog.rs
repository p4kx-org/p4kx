// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shared exact original value contents for physical owner-local Catalog inventories.

#[path = "source_arena.rs"]
mod arena;
#[path = "source_container.rs"]
mod container;
#[path = "source_data_type.rs"]
mod data_type;
#[path = "original_value_error.rs"]
mod error;
#[cfg(test)]
#[path = "original_value_error_cases.rs"]
mod error_cases;
#[path = "source_error_facts.rs"]
mod error_facts;
#[path = "source_error_read.rs"]
mod error_read;
#[path = "source_field.rs"]
mod field;
#[path = "source_field_read.rs"]
mod field_read;
#[path = "source_reference.rs"]
mod reference;
#[path = "source_reference_read.rs"]
mod reference_read;
#[path = "source_scalar_read.rs"]
mod scalar_read;
#[path = "source_text.rs"]
mod text;
#[path = "source_value.rs"]
mod value;

pub(crate) use arena::{SourceArenaOutput, read as read_arena, read_indexed as read_indexed_arena};
pub(crate) use field::SourceField;
pub(crate) use field_read::read as read_field;
pub(crate) use reference::{SourceRecordReference, SourceWeakReference};
pub(crate) use reference_read::read as read_reference_payload;
pub(crate) use scalar_read::{read as read_scalar, read_reference};
pub(crate) use value::{SourceNode, SourcePayload, SourceValueRef};
