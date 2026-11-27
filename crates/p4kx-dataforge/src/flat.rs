// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "flat_observation.rs"]
mod observation;
#[path = "flat_output.rs"]
mod output;
#[path = "flat_render.rs"]
mod render;
#[path = "flat_task.rs"]
mod task;
#[path = "flat_walk.rs"]
mod walk;
#[path = "flat_writer.rs"]
mod writer;

pub use observation::{FlatColumn, FlatColumnSpan, FlatRowObservation};
pub use writer::{FLAT_COLUMNS, FLAT_TAG_COLUMN, FLAT_TEXT_COLUMN, FlatFormat, FlatValues};
