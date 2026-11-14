// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use crate::HealthText;

/// Shared descriptive spellings for source-neutral interpreted accounting.
#[derive(Default)]
pub(crate) struct CatalogReferenceTexts {
    values: BTreeSet<HealthText>,
}

impl CatalogReferenceTexts {
    pub(crate) fn retain(&mut self, value: &str) -> HealthText {
        if let Some(retained) = self.values.get(value) {
            return retained.clone();
        }
        let retained = HealthText::new(value);
        self.values.insert(retained.clone());
        retained
    }
}
