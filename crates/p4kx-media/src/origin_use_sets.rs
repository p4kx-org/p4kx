// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::{DecimalU64, SourceUseRef};
use serde::{Deserialize, Serialize};

use crate::origin_contributions::SharedUseSet;
use crate::{MediaError, origin_error};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub(crate) struct SetId(
    #[serde(deserialize_with = "crate::origin_decimal::deserialize")] pub(crate) DecimalU64,
);

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(crate) enum SetRow<R> {
    Set {
        id: SetId,
        #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
        members: DecimalU64,
    },
    Member {
        #[serde(rename = "sourceUseRef")]
        reference: R,
    },
    End {
        id: SetId,
    },
    Eof {
        #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
        sets: DecimalU64,
        #[serde(deserialize_with = "crate::origin_decimal::deserialize")]
        members: DecimalU64,
    },
}

#[derive(Default)]
pub(crate) struct Sets {
    order: Vec<Arc<SharedUseSet>>,
    registered: HashMap<usize, SetId>,
}

impl Sets {
    pub(crate) fn register(&mut self, shared: &Arc<SharedUseSet>) -> Result<SetId, MediaError> {
        let identity = Arc::as_ptr(shared).addr();
        if let Some(id) = self.registered.get(&identity) {
            return Ok(*id);
        }
        let id = SetId(count(self.order.len())?);
        self.order.push(Arc::clone(shared));
        self.registered.insert(identity, id);
        Ok(id)
    }

    pub(crate) fn visit(
        &self,
        mut write: impl FnMut(SetRow<&SourceUseRef>) -> Result<(), MediaError>,
    ) -> Result<(), MediaError> {
        let mut members = 0_u64;
        for (index, shared) in self.order.iter().enumerate() {
            let id = SetId(count(index)?);
            let entries = count(shared.uses.len())?;
            write(SetRow::Set {
                id,
                members: entries,
            })?;
            for member in shared.uses.iter() {
                write(SetRow::Member {
                    reference: member.reference(),
                })?;
            }
            write(SetRow::End { id })?;
            members = members
                .checked_add(entries.get())
                .ok_or_else(origin_error::counter)?;
        }
        write(SetRow::Eof {
            sets: count(self.order.len())?,
            members: DecimalU64::new(members),
        })
    }
}

fn count(value: usize) -> Result<DecimalU64, MediaError> {
    u64::try_from(value)
        .map(DecimalU64::new)
        .map_err(|_| origin_error::counter())
}

#[cfg(test)]
#[path = "origin_use_sets_tests.rs"]
mod tests;
