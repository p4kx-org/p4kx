// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::fault::Fault;
use crate::installer::lease::Lease;
use crate::installer::path_edit;
use crate::installer::path_edit::Insertion;
use crate::installer::raw_path::RawPath;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Claim {
    Borrowed,
    Inserted(Insertion),
}

pub(crate) struct Acquisition {
    pub(crate) path: RawPath,
    pub(crate) claim: Claim,
}

pub(crate) fn acquire(path: &RawPath, lease: &Lease) -> Result<Acquisition, Fault> {
    if let Some(parts) = path.split()?
        && path_edit::locate(parts.body, lease)?.is_some()
    {
        return Ok(Acquisition {
            path: path.clone(),
            claim: Claim::Borrowed,
        });
    }
    let insertion = path_edit::insert(path, lease)?;
    Ok(Acquisition {
        path: insertion.after.clone(),
        claim: Claim::Inserted(insertion),
    })
}

pub(crate) fn remove(path: &RawPath, lease: &Lease, claim: &Claim) -> Result<RawPath, Fault> {
    match claim {
        Claim::Borrowed => Ok(path.clone()),
        Claim::Inserted(insertion) => path_edit::remove(path, lease, insertion),
    }
}
