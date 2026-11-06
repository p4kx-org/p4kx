// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{Archive, DataForgeEntry, Guid};

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_identity::LogicalKey;
use super::reference_manifest::{DcbIdentity, MAX_LOGICAL_SOURCES};
use super::reference_paths;

pub(crate) struct Records {
    pub(crate) dcb: DcbIdentity,
    pub(crate) guids: BTreeSet<Guid>,
    pub(crate) universe: BTreeSet<LogicalKey>,
}

fn guids(archive: &Archive) -> Result<BTreeSet<Guid>> {
    let loaded = DataForgeEntry::read(archive)
        .map_err(|error| ReferenceError::new(Code::Provenance, error.to_string()))?;
    let forge = loaded
        .open()
        .map_err(|error| ReferenceError::new(Code::Provenance, error.to_string()))?;
    if forge.record_count() > MAX_LOGICAL_SOURCES {
        return Err(ReferenceError::new(
            Code::Bounds,
            "DCB source inventory exceeds its bound",
        ));
    }
    let mut guids = BTreeSet::new();
    for record in forge.records() {
        if !guids.insert(record.guid()) {
            return Err(ReferenceError::new(
                Code::Identity,
                "DCB contains duplicate source GUIDs",
            ));
        }
    }
    Ok(guids)
}

pub(crate) fn measure(archive: &Archive) -> Result<Records> {
    let entry = DataForgeEntry::locate(archive)
        .ok_or_else(|| ReferenceError::new(Code::Provenance, "source Archive has no DCB"))?;
    let reader = archive
        .open_entry(entry)
        .map_err(|error| ReferenceError::new(Code::Provenance, error.to_string()))?;
    let fingerprint = reference_digest::reader(reader, p4kx_extraction::MAX_DATAFORGE_SIZE)?;
    let dcb_key = reference_paths::key(entry.path())?;
    let guids = guids(archive)?;
    let universe = guids
        .iter()
        .map(|guid| LogicalKey::Record {
            guid: *guid,
            dcb: dcb_key.clone(),
        })
        .collect();
    Ok(Records {
        dcb: DcbIdentity {
            entry: entry.path().to_owned(),
            fingerprint,
        },
        guids,
        universe,
    })
}
