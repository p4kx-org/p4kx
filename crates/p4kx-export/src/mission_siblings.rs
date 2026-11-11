// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Filter};

use crate::CatalogError;
use crate::broker_entry_read::{self, BrokerEntries};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::contract_template_read::{self, ContractTemplates};
use crate::location_template::{self, LocationTemplates};
use crate::mission_index::MissionIndex;

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
    projection: &CatalogProjection<'_, 'f>,
    (captured_tags, index): (&CapturedTags<'_, 'f>, &MissionIndex<'f>),
    filter: Option<&Filter>,
    cancel: Option<&CancelToken>,
) -> Result<Siblings<'f>, CatalogError> {
    let input = projection.original();
    let texts = projection.legacy_texts();
    let tags = index.tags();
    let contract_templates =
        contract_template_read::read(forge, projection, captured_tags, filter, cancel)?;
    let broker_entries = broker_entry_read::read(
        forge,
        projection,
        (captured_tags, index, &contract_templates.unknowns),
        filter,
        cancel,
    )?;
    Ok(Siblings {
        location_templates: location_template::read(forge, input, texts, tags, filter, cancel)?,
        contract_templates,
        broker_entries,
    })
}
