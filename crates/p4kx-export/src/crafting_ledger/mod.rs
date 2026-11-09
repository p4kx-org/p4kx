// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod assemble;
mod fault;
mod mode;
mod projection;
mod read;
mod resource_scope;
mod selection;

use crate::catalog_v3::gameplay_property_catalog::GameplayPropertyRows;
use crate::export_ledger::ExportLedger;

use super::Crafting;

pub(crate) use mode::ReadMode;
pub(crate) use read::{read, read_catalog};

/// The public crafting result and actual production-source observations.
pub(crate) struct ReadCrafting<'f> {
    pub(crate) data: Crafting<'f>,
    pub(crate) ledger: ExportLedger,
    pub(crate) gameplay: GameplayPropertyRows<'f>,
    pub(crate) quality: crate::quality_catalog::QualityRows<'f>,
}

#[cfg(test)]
pub(crate) mod fixtures;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod policy_tests;
