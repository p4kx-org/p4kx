// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::binding::{self, Binding};
use crate::installer::caller::{Caller, Goal};
use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::invocation_nonce::InvocationNonce;
use crate::installer::journal_codec;
use crate::installer::lease::Lease;
use crate::installer::path_edit::Insertion;
use crate::installer::raw_path::{self, RawPath, StringKind};
use crate::installer::reservation::Reservation;
use crate::installer::rollback_receipt::{RollbackReceipt, RollbackSource};
use crate::installer::state::{Journal, Ownership, Phase, Snapshot};

pub(crate) fn snapshot(bytes: &[u8]) -> Result<Snapshot, Fault> {
    if bytes.len() > journal_codec::MAX_STATE_BYTES {
        return Err(Fault::StateLimit);
    }
    let mut reader = Reader { remaining: bytes };
    if reader.take(journal_codec::MAGIC.len())? != journal_codec::MAGIC {
        return Err(Fault::CorruptState);
    }
    let sequence = u64::from_le_bytes(reader.array()?);
    let path = reader.path()?;
    let owner = reader.owner()?;
    let journal = reader.journal()?;
    let reservation = reader.reservation()?;
    let cancelled_binding = reader.cancelled()?;
    if !reader.remaining.is_empty() {
        return Err(Fault::CorruptState);
    }
    Ok(Snapshot {
        reservation,
        cancelled_binding,
        path,
        owner,
        journal,
        sequence,
    })
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Fault> {
        let result = self.remaining.get(..count).ok_or(Fault::CorruptState)?;
        self.remaining = self.remaining.get(count..).ok_or(Fault::CorruptState)?;
        Ok(result)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Fault> {
        self.take(N)?.try_into().map_err(|_| Fault::CorruptState)
    }

    fn tag(&mut self) -> Result<u8, Fault> {
        let [tag] = self.array()?;
        Ok(tag)
    }

    fn blob(&mut self) -> Result<Vec<u8>, Fault> {
        self.bounded_blob(raw_path::MAX_PATH_BYTES)
    }

    fn bounded_blob(&mut self, maximum: usize) -> Result<Vec<u8>, Fault> {
        let count =
            usize::try_from(u32::from_le_bytes(self.array()?)).map_err(|_| Fault::StateLimit)?;
        if count > maximum {
            return Err(Fault::StateLimit);
        }
        Ok(self.take(count)?.to_vec())
    }

    fn path(&mut self) -> Result<RawPath, Fault> {
        let kind = match self.tag()? {
            0 => return Ok(RawPath::Absent),
            1 => StringKind::Plain,
            2 => StringKind::Expandable,
            _ => return Err(Fault::CorruptState),
        };
        let path = RawPath::Present {
            kind,
            bytes: self.blob()?,
        };
        path.split()?;
        Ok(path)
    }

    fn owner(&mut self) -> Result<Option<Ownership>, Fault> {
        match self.tag()? {
            0 => return Ok(None),
            1 => {}
            _ => return Err(Fault::CorruptState),
        }
        let product = u128::from_le_bytes(self.array()?);
        let package = u128::from_le_bytes(self.array()?);
        let lease = Lease::try_new(self.blob()?)?;
        let claim = match self.tag()? {
            0 => Claim::Borrowed,
            1 => Claim::Inserted(Insertion {
                before: self.path()?,
                after: self.path()?,
            }),
            _ => return Err(Fault::CorruptState),
        };
        Ok(Some(Ownership {
            product,
            package,
            lease,
            claim,
        }))
    }

    fn journal(&mut self) -> Result<Option<Journal>, Fault> {
        match self.tag()? {
            0 => return Ok(None),
            1 => {}
            _ => return Err(Fault::CorruptState),
        }
        let id = u64::from_le_bytes(self.array()?);
        let phase = match self.tag()? {
            0 => Phase::Prepared,
            1 => Phase::Applied,
            2 => Phase::Committed,
            3 => Phase::RolledBack,
            _ => return Err(Fault::CorruptState),
        };
        let caller = self.caller()?;
        Ok(Some(Journal {
            id,
            phase,
            caller,
            before_path: self.path()?,
            after_path: self.path()?,
            before_owner: self.owner()?,
            after_owner: self.owner()?,
            rollback: self.rollback()?,
        }))
    }

    fn rollback(&mut self) -> Result<Option<RollbackReceipt>, Fault> {
        match self.tag()? {
            0 => return Ok(None),
            1 => {}
            _ => return Err(Fault::CorruptState),
        }
        let from = match self.tag()? {
            0 => RollbackSource::Prepared,
            1 => RollbackSource::Applied,
            2 => RollbackSource::Committed,
            _ => return Err(Fault::CorruptState),
        };
        Ok(Some(RollbackReceipt {
            from,
            observed_path: self.path()?,
        }))
    }

    fn cancelled(&mut self) -> Result<Option<Binding>, Fault> {
        match self.tag()? {
            0 => return Ok(None),
            1 => {}
            _ => return Err(Fault::CorruptState),
        }
        Ok(Some(Binding::from_registry(
            &self.bounded_blob(binding::MAX_REGISTRY_BYTES)?,
        )?))
    }

    fn reservation(&mut self) -> Result<Option<Reservation>, Fault> {
        match self.tag()? {
            0 => return Ok(None),
            1 => {}
            _ => return Err(Fault::CorruptState),
        }
        Ok(Some(Reservation {
            base_sequence: u64::from_le_bytes(self.array()?),
            caller: self.caller()?,
        }))
    }

    fn caller(&mut self) -> Result<Caller, Fault> {
        let product = u128::from_le_bytes(self.array()?);
        let package = u128::from_le_bytes(self.array()?);
        let nonce = InvocationNonce::from_bytes(self.array()?)?;
        let predecessor = match self.tag()? {
            0 => None,
            1 => Some(u128::from_le_bytes(self.array()?)),
            _ => return Err(Fault::CorruptState),
        };
        let goal = match self.tag()? {
            0 => Goal::Remove,
            1 => Goal::Ensure(Lease::try_new(self.blob()?)?),
            _ => return Err(Fault::CorruptState),
        };
        let caller = Caller {
            product,
            package,
            nonce,
            predecessor,
            goal,
        };
        caller.validate()?;
        Ok(caller)
    }
}
