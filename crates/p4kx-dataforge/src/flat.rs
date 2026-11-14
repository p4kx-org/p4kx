// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "flat_task.rs"]
mod task;
#[path = "flat_walk.rs"]
mod walk;
#[path = "flat_writer.rs"]
mod writer;

pub use writer::{FLAT_COLUMNS, FLAT_TAG_COLUMN, FLAT_TEXT_COLUMN, FlatFormat, FlatValues};
