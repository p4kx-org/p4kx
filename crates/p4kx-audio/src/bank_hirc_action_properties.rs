// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::bank_node_property_rows::BankNodePropertyRows;
use crate::hirc_action::BankPlayProperties;
use crate::hirc_node_property::BankNodePropertyKind;
use crate::read_at::SeekSource;

use super::BankHircAction;

/// A version-154 Play action and its property rows bound to the original bank.
pub struct BankHircActionProperties<'input, R> {
    input: &'input mut SeekSource<R>,
    action: BankHircAction,
    fields: BankPlayProperties,
}

impl<'input, R> BankHircActionProperties<'input, R> {
    pub(super) const fn new(
        input: &'input mut SeekSource<R>,
        action: BankHircAction,
        fields: BankPlayProperties,
    ) -> Self {
        Self {
            input,
            action,
            fields,
        }
    }

    /// Returns the original Action interpretation.
    #[must_use]
    pub const fn action(&self) -> &BankHircAction {
        &self.action
    }

    /// Returns the original Play flags and checked planar list extents.
    #[must_use]
    pub const fn fields(&self) -> &BankPlayProperties {
        &self.fields
    }

    /// Borrows checked original IDs and words from the selected property list.
    pub fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        self.fields
            .lists
            .get(kind)
            .map(|extent| BankNodePropertyRows::new(self.input, extent))
    }
}
