// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::Identity;
use super::reference_paths;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum LogicalKey {
    Entry(String),
    Record { guid: Guid, dcb: String },
}

pub(crate) fn key(identity: &Identity) -> Result<LogicalKey> {
    match identity {
        Identity::Entry { path } => Ok(LogicalKey::Entry(reference_paths::key(path)?)),
        Identity::Record { guid, dcb } => {
            let guid_text = guid;
            let guid = guid
                .parse::<Guid>()
                .map_err(|error| ReferenceError::new(Code::Identity, error.to_string()))?;
            if guid.to_string() != *guid_text {
                return Err(ReferenceError::new(
                    Code::Identity,
                    "Record GUID is not canonical",
                ));
            }
            Ok(LogicalKey::Record {
                guid,
                dcb: reference_paths::key(dcb)?,
            })
        }
    }
}
