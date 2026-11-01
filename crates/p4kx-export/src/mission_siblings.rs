// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};

use crate::CatalogError;
use crate::broker_entry_read::{self, BrokerEntries};
use crate::contract_template_read::{self, ContractTemplates};
use crate::location_template::{self, LocationTemplates};
use crate::mission_index::MissionIndex;
use crate::texts::Texts;

/// The kinds read beside the contracts: the mission location templates, the contract templates
/// and the mission broker entries.
pub(crate) struct Siblings<'f> {
    pub(crate) location_templates: LocationTemplates<'f>,
    pub(crate) contract_templates: ContractTemplates<'f>,
    pub(crate) broker_entries: BrokerEntries<'f>,
}

/// Reads the sibling kinds `filter` matches. The contract templates are read first: their
/// `Unknowns` is the one the broker entries are read with, so both count the stored names they do
/// not know in one place, and the caller takes it from `contract_templates` afterwards.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn read<'f>(
    forge: &'f DataForge<'f>,
    (texts, index): (&Texts<'f>, &MissionIndex<'f>),
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Siblings<'f>, CatalogError> {
    let tags = index.tags();
    let contract_templates = contract_template_read::read(forge, texts, tags, filter, cancel)?;
    let broker_entries = broker_entry_read::read(
        forge,
        (texts, index, &contract_templates.unknowns),
        filter,
        cancel,
    )?;
    Ok(Siblings {
        location_templates: location_template::read(forge, texts, tags, filter, cancel)?,
        contract_templates,
        broker_entries,
    })
}
