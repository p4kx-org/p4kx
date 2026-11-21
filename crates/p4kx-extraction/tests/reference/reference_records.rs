// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{Archive, DataForgeEntry, Guid};

use super::reference_digest;
use super::reference_error::{Code, ReferenceError, Result};
use super::reference_manifest::DcbIdentity;

pub(crate) struct Records {
    pub(crate) dcb: DcbIdentity,
    pub(crate) guids: BTreeSet<Guid>,
}

fn guids(archive: &Archive) -> Result<BTreeSet<Guid>> {
    let loaded = DataForgeEntry::read(archive)
        .map_err(|error| ReferenceError::new(Code::Provenance, error.to_string()))?;
    let forge = loaded
        .open()
        .map_err(|error| ReferenceError::new(Code::Provenance, error.to_string()))?;
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
    let guids = guids(archive)?;
    Ok(Records {
        dcb: DcbIdentity {
            entry: entry.path().to_owned(),
            fingerprint,
        },
        guids,
    })
}
