// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::installer::caller::{Caller, Goal};
use crate::installer::claim::Claim;
use crate::installer::fault::Fault;
use crate::installer::journal_codec;
use crate::installer::raw_path::{RawPath, StringKind};
use crate::installer::rollback_receipt::{RollbackReceipt, RollbackSource};
use crate::installer::state::{Journal, Ownership, Phase, Snapshot};

pub(crate) fn snapshot(snapshot: &Snapshot) -> Result<Vec<u8>, Fault> {
    let mut writer = Writer { bytes: Vec::new() };
    writer.put(journal_codec::MAGIC)?;
    writer.put(&snapshot.sequence.to_le_bytes())?;
    writer.path(&snapshot.path)?;
    writer.owner(snapshot.owner.as_ref())?;
    writer.journal(snapshot.journal.as_ref())?;
    Ok(writer.bytes)
}

struct Writer {
    bytes: Vec<u8>,
}

impl Writer {
    fn put(&mut self, source: &[u8]) -> Result<(), Fault> {
        let size = self
            .bytes
            .len()
            .checked_add(source.len())
            .ok_or(Fault::StateLimit)?;
        if size > journal_codec::MAX_STATE_BYTES {
            return Err(Fault::StateLimit);
        }
        self.bytes.extend_from_slice(source);
        Ok(())
    }

    fn blob(&mut self, source: &[u8]) -> Result<(), Fault> {
        let length = u32::try_from(source.len()).map_err(|_| Fault::StateLimit)?;
        self.put(&length.to_le_bytes())?;
        self.put(source)
    }

    fn path(&mut self, path: &RawPath) -> Result<(), Fault> {
        match path {
            RawPath::Absent => self.put(&[0]),
            RawPath::Present { kind, bytes } => {
                self.put(&[match kind {
                    StringKind::Plain => 1,
                    StringKind::Expandable => 2,
                }])?;
                self.blob(bytes)
            }
        }
    }

    fn owner(&mut self, owner: Option<&Ownership>) -> Result<(), Fault> {
        let Some(owner) = owner else {
            return self.put(&[0]);
        };
        self.put(&[1])?;
        self.put(&owner.product.to_le_bytes())?;
        self.put(&owner.package.to_le_bytes())?;
        self.blob(&owner.lease.entry)?;
        match &owner.claim {
            Claim::Borrowed => self.put(&[0]),
            Claim::Inserted(insertion) => {
                self.put(&[1])?;
                self.path(&insertion.before)?;
                self.path(&insertion.after)
            }
        }
    }

    fn journal(&mut self, record: Option<&Journal>) -> Result<(), Fault> {
        let Some(record) = record else {
            return self.put(&[0]);
        };
        self.put(&[1])?;
        self.put(&record.id.to_le_bytes())?;
        self.put(&[match record.phase {
            Phase::Prepared => 0,
            Phase::Applied => 1,
            Phase::Committed => 2,
            Phase::RolledBack => 3,
        }])?;
        self.caller(&record.caller)?;
        self.path(&record.before_path)?;
        self.path(&record.after_path)?;
        self.owner(record.before_owner.as_ref())?;
        self.owner(record.after_owner.as_ref())?;
        self.rollback(record.rollback.as_ref())
    }

    fn rollback(&mut self, receipt: Option<&RollbackReceipt>) -> Result<(), Fault> {
        let Some(receipt) = receipt else {
            return self.put(&[0]);
        };
        self.put(&[1])?;
        self.put(&[match receipt.from {
            RollbackSource::Prepared => 0,
            RollbackSource::Applied => 1,
            RollbackSource::Committed => 2,
        }])?;
        self.path(&receipt.observed_path)
    }

    fn caller(&mut self, caller: &Caller) -> Result<(), Fault> {
        self.put(&caller.product.to_le_bytes())?;
        self.put(&caller.package.to_le_bytes())?;
        self.put(caller.nonce.bytes())?;
        if let Some(previous) = caller.predecessor {
            self.put(&[1])?;
            self.put(&previous.to_le_bytes())?;
        } else {
            self.put(&[0])?;
        }
        match &caller.goal {
            Goal::Remove => self.put(&[0]),
            Goal::Ensure(lease) => {
                self.put(&[1])?;
                self.blob(&lease.entry)
            }
        }
    }
}
