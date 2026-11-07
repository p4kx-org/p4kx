// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::claim::{self, Claim};
use crate::installer::fault::Fault;
use crate::installer::path_edit;
use crate::installer::raw_path::RawPath;
use crate::installer::rollback_receipt::{RollbackReceipt, RollbackSource};
use crate::installer::state::{Journal, Ownership};

pub(crate) struct Compensation {
    pub(crate) path: RawPath,
    pub(crate) owner: Option<Ownership>,
}

pub(crate) fn project(record: &Journal, receipt: &RollbackReceipt) -> Result<Compensation, Fault> {
    let current = &receipt.observed_path;
    current.split()?;
    if receipt.from == RollbackSource::Prepared {
        return Ok(Compensation {
            path: current.clone(),
            owner: record.before_owner.clone(),
        });
    }
    if current == &record.after_path {
        return Ok(Compensation {
            path: record.before_path.clone(),
            owner: record.before_owner.clone(),
        });
    }
    if record.before_path == record.after_path {
        return Ok(Compensation {
            path: current.clone(),
            owner: record.before_owner.clone(),
        });
    }
    if let Some(owner) = &record.after_owner {
        let path = claim::remove(current, &owner.lease, &owner.claim)?;
        return restore_previous(path, record);
    }
    let mut owner = record.before_owner.clone().ok_or(Fault::WrongOwner)?;
    let acquired = claim::acquire(current, &owner.lease)?;
    owner.claim = acquired.claim;
    Ok(Compensation {
        path: acquired.path,
        owner: Some(owner),
    })
}

fn restore_previous(path: RawPath, record: &Journal) -> Result<Compensation, Fault> {
    let Some(mut owner) = record.before_owner.clone() else {
        return Ok(Compensation { path, owner: None });
    };
    if owner.claim == Claim::Borrowed {
        return Ok(Compensation {
            path,
            owner: Some(owner),
        });
    }
    let had_lease = record
        .before_path
        .split()?
        .map(|parts| path_edit::locate(parts.body, &owner.lease))
        .transpose()?
        .flatten()
        .is_some();
    if !had_lease {
        return Ok(Compensation {
            path,
            owner: Some(owner),
        });
    }
    let acquired = claim::acquire(&path, &owner.lease)?;
    owner.claim = acquired.claim;
    Ok(Compensation {
        path: acquired.path,
        owner: Some(owner),
    })
}
