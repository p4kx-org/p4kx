// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataForge;

use crate::broker_entry_read::{self, BrokerEntries};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::contract_template_read::{self, ContractTemplates};
use crate::export_field_inspection::{FieldInspection, WholeInspectionFold};
use crate::location_template::{self, LocationTemplates};
use crate::mission_index::MissionIndex;
use crate::options::{CatalogRun, CatalogStage};
use crate::{CatalogError, Kind};

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
    run: CatalogRun<'_, '_>,
    dependencies: &mut [FieldInspection],
) -> Result<Siblings<'f>, CatalogError> {
    let filter = run.options.filter;
    let cancel = run.options.cancel;
    let mut inspections = WholeInspectionFold::new(dependencies);
    run.begin(CatalogStage::ProjectContractTemplates);
    let contract_templates = contract_template_read::read(
        forge,
        &projection.for_kind(Kind::ContractTemplates),
        captured_tags,
        filter,
        cancel,
        &mut inspections,
    )?;
    run.begin(CatalogStage::ProjectMissionBrokerEntries);
    let broker_entries = broker_entry_read::read(
        forge,
        &projection.for_kind(Kind::MissionBrokerEntries),
        (captured_tags, index, &contract_templates.unknowns),
        filter,
        cancel,
        &mut inspections,
    )?;
    run.begin(CatalogStage::ProjectMissionLocationTemplates);
    let location_templates = location_template::read(
        forge,
        &projection.for_kind(Kind::MissionLocationTemplates),
        index.tags(),
        filter,
        cancel,
    )?;
    Ok(Siblings {
        location_templates,
        contract_templates,
        broker_entries,
    })
}
