// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod assemble;
mod fault;
mod projection;
mod read;
mod selection;

use crate::export_ledger::ExportLedger;

use super::Crafting;

pub(crate) use read::read;

/// The public crafting result and actual production-source observations.
pub(crate) struct ReadCrafting<'f> {
    pub(crate) data: Crafting<'f>,
    pub(crate) ledger: ExportLedger,
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod policy_tests;
