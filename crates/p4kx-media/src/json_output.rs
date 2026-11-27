// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use crate::checkpoint::Checkpoint;
use crate::{MediaError, error, output_operation};

/// Completes this job's guarded JSON writer result without erasing an independent failure.
/// Only unpositioned I/O from the owned `ObservedWrite` serialisation seams is unwrapped.
pub(crate) fn finish(
    serialised: Result<(), serde_json::Error>,
    checkpoint: Checkpoint<'_>,
) -> Result<(), MediaError> {
    let operation = serialised.map_err(cause);
    let postcheck = checkpoint.check();
    if let (Err(cause), Err(post)) = (&operation, &postcheck)
        && repeated_stop(cause, post, checkpoint)
    {
        return operation;
    }
    output_operation::checked(operation, postcheck)
}

fn cause(error: serde_json::Error) -> MediaError {
    if error.classify() == serde_json::error::Category::Io
        && error.line() == 0
        && error.column() == 0
    {
        error::input(error::io(Path::new("media JSON writer"), error.into()))
    } else {
        MediaError::Json(error)
    }
}

fn repeated_stop(
    operation: &MediaError,
    postcheck: &MediaError,
    checkpoint: Checkpoint<'_>,
) -> bool {
    match (operation, postcheck) {
        (MediaError::Cancelled, MediaError::Cancelled) => checkpoint.cancel.is_cancelled(),
        (MediaError::Changed(original), MediaError::Changed(current)) => {
            checkpoint.watch.is_changed() && original.path() == current.path()
        }
        _ => false,
    }
}

#[cfg(test)]
#[path = "json_output_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "json_output_position_tests.rs"]
mod position_tests;
