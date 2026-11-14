// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "manufacturer_definition.rs"]
mod manufacturer_definition;
#[path = "manufacturer_read.rs"]
mod manufacturer_read;

pub(crate) use manufacturer_definition::{Manufacturer, ManufacturerDefinition};
pub(crate) use manufacturer_read::{read, table};
