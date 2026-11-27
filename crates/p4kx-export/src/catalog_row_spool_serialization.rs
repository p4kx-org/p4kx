// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::CancelToken;
use serde::ser::SerializeSeq as _;
use serde::{Serialize, Serializer};
use serde_json::value::RawValue;

use super::{ReplayError, RowSpool, RowSpoolError};

impl RowSpool {
    pub(crate) const fn serialize_rows<'a>(
        &'a self,
        cancel: Option<&'a CancelToken>,
    ) -> SpoolRows<'a> {
        SpoolRows {
            spool: self,
            cancel,
            failure: RefCell::new(None),
        }
    }
}

/// Serializes the same ordered row sequence on each invocation.
#[derive(Debug)]
pub(crate) struct SpoolRows<'a> {
    spool: &'a RowSpool,
    cancel: Option<&'a CancelToken>,
    failure: RefCell<Option<RowSpoolError>>,
}

impl SpoolRows<'_> {
    /// Recovers the typed input error after a failed serialization.
    pub(crate) fn take_failure(&self) -> Option<RowSpoolError> {
        self.failure.take()
    }

    /// Streams verified rows into an existing JSON sequence.
    pub(crate) fn visit<E: serde::ser::Error>(
        &self,
        visit: impl FnMut(&RawValue) -> Result<(), E>,
    ) -> Result<(), E> {
        *self.failure.borrow_mut() = None;
        match self.spool.replay(self.cancel, visit) {
            Ok(()) => Ok(()),
            Err(ReplayError::Output(error)) => Err(error),
            Err(ReplayError::Spool(error)) => {
                let reported = E::custom(&error);
                *self.failure.borrow_mut() = Some(error);
                Err(reported)
            }
        }
    }
}

impl Serialize for SpoolRows<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        *self.failure.borrow_mut() = None;
        let mut sequence = serializer.serialize_seq(Some(self.spool.rows()))?;
        self.visit(|row| sequence.serialize_element(row))?;
        sequence.end()
    }
}
