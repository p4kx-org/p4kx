// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::claim::Claim;
use crate::installer::compensation;
use crate::installer::fault::Fault;
use crate::installer::journal_decode;
use crate::installer::journal_encode;
use crate::installer::path_edit;
use crate::installer::state::{Ownership, Phase, Snapshot};
use crate::installer::transaction;

pub(crate) const MAGIC: &[u8] = b"P4KXPATH\x02";
pub(crate) const MAX_STATE_BYTES: usize = 16 << 20;

pub(crate) fn encode(snapshot: &Snapshot) -> Result<Vec<u8>, Fault> {
    validate(snapshot)?;
    journal_encode::snapshot(snapshot)
}

pub(crate) fn decode(bytes: &[u8]) -> Result<Snapshot, Fault> {
    let snapshot = journal_decode::snapshot(bytes)?;
    validate(&snapshot)?;
    Ok(snapshot)
}

fn validate(snapshot: &Snapshot) -> Result<(), Fault> {
    snapshot.path.split()?;
    owner(snapshot.owner.as_ref())?;
    let Some(record) = &snapshot.journal else {
        return if snapshot.sequence == 0 && snapshot.owner.is_none() {
            Ok(())
        } else {
            Err(Fault::CorruptState)
        };
    };
    if record.id != snapshot.sequence || record.id == 0 {
        return Err(Fault::CorruptState);
    }
    record.caller.validate()?;
    transaction::validate_record(record)?;
    record.before_path.split()?;
    record.after_path.split()?;
    owner(record.before_owner.as_ref())?;
    owner(record.after_owner.as_ref())?;
    let expected = match record.phase {
        Phase::Prepared => &record.before_owner,
        Phase::Applied | Phase::Committed => &record.after_owner,
        Phase::RolledBack => {
            let receipt = record.rollback.as_ref().ok_or(Fault::CorruptState)?;
            let projected = compensation::project(record, receipt)?;
            projected.path.split()?;
            owner(projected.owner.as_ref())?;
            return if snapshot.owner == projected.owner {
                Ok(())
            } else {
                Err(Fault::CorruptState)
            };
        }
    };
    if record.rollback.is_none() && &snapshot.owner == expected {
        Ok(())
    } else {
        Err(Fault::CorruptState)
    }
}

fn owner(owner: Option<&Ownership>) -> Result<(), Fault> {
    let Some(owner) = owner else {
        return Ok(());
    };
    if owner.product == 0 || owner.package == 0 {
        return Err(Fault::CorruptState);
    }
    let Claim::Inserted(insertion) = &owner.claim else {
        return Ok(());
    };
    let canonical = path_edit::insert(&insertion.before, &owner.lease)?;
    if &canonical == insertion {
        Ok(())
    } else {
        Err(Fault::CorruptState)
    }
}

#[cfg(test)]
#[path = "journal_tests.rs"]
mod tests;
