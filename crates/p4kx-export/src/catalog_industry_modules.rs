// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(super) use super::{authored_fields, prepared_definition};

#[path = "crafting_global.rs"]
pub(crate) mod crafting_global;
#[path = "mineable_composition_definition.rs"]
pub(crate) mod mineable_composition_definition;
#[path = "mineable_element_definition.rs"]
pub(crate) mod mineable_element_definition;
#[path = "mining_process_catalog.rs"]
pub(crate) mod mining_process_catalog;
