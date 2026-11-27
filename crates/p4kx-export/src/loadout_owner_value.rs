// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::{Serialize, Serializer};

/// A required owner value with its original contribution set.
#[derive(Debug)]
pub(crate) struct LoadoutOwnerValue<T> {
    pub(crate) value: T,
    evidence: SourcedValue<()>,
}

impl<T> LoadoutOwnerValue<T> {
    pub(crate) const fn new(value: T, evidence: SourcedValue<()>) -> Self {
        Self { value, evidence }
    }

    pub(crate) fn sourced(&self) -> SourcedValue<&T> {
        self.evidence.map_ref(|()| &self.value)
    }

    #[cfg(test)]
    pub(crate) fn fixture(value: T) -> Self {
        use p4kx_extraction::{SourceUses, SourceValue};

        Self::new(
            value,
            SourceUses::default().value(SourceValue::Present(()), &[], false),
        )
    }
}

impl<T: Serialize> Serialize for LoadoutOwnerValue<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.sourced().serialize(serializer)
    }
}
