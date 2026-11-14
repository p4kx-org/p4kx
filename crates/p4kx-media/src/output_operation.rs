// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::MediaError;

pub(crate) fn checked<T>(
    operation: Result<T, MediaError>,
    checkpoint: Result<(), MediaError>,
) -> Result<T, MediaError> {
    match (operation, checkpoint) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(cause), Ok(())) | (Ok(_), Err(cause)) => Err(cause),
        (Err(operation), Err(checkpoint)) => Err(MediaError::OutputCheckFailed {
            operation: Box::new(operation),
            checkpoint: Box::new(checkpoint),
        }),
    }
}
