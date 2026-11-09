// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, OriginReaderLimits, SourceUseRef};

use crate::MediaError;
use crate::origin_contract::OriginReceipts;

/// Finite consumer storage policy independent of producer admission.
#[derive(Debug, Clone, Copy)]
pub(crate) struct OriginReadPolicy {
    pub(crate) line_bytes: u64,
    pub(crate) inventory_bytes: u64,
    pub(crate) fields: OriginReaderLimits,
}

impl OriginReadPolicy {
    pub(crate) fn from_receipts(receipts: &OriginReceipts) -> Self {
        let line_bytes = receipts
            .sources
            .size
            .max(receipts.uses.size)
            .max(receipts.fields.size);
        let inventory_bytes = receipts
            .sources
            .size
            .saturating_add(receipts.uses.size)
            .saturating_mul((1 + size_of::<FieldOrigin>() + size_of::<SourceUseRef>()) as u64);
        let field_bytes = usize::try_from(line_bytes).unwrap_or(usize::MAX);
        Self {
            line_bytes,
            inventory_bytes,
            fields: OriginReaderLimits {
                fields: field_bytes,
                metadata_bytes: field_bytes
                    .saturating_mul(1 + size_of::<SourceUseRef>())
                    .saturating_add(size_of::<FieldOrigin>()),
                path_bytes: field_bytes,
                field_uses: field_bytes,
                reference_bytes: field_bytes,
            },
        }
    }
}

/// Tracks only retained descriptive inventory storage.
pub(crate) struct InventoryBudget {
    remaining: u64,
}

impl InventoryBudget {
    pub(crate) const fn new(policy: OriginReadPolicy) -> Self {
        Self {
            remaining: policy.inventory_bytes,
        }
    }

    pub(crate) fn charge(&mut self, bytes: usize) -> Result<(), MediaError> {
        let bytes = u64::try_from(bytes).map_err(|_| {
            MediaError::Destination("inventory storage is not representable".into())
        })?;
        self.remaining = self.remaining.checked_sub(bytes).ok_or_else(|| {
            MediaError::Destination("origin inventory exceeds the reader storage policy".into())
        })?;
        Ok(())
    }
}
