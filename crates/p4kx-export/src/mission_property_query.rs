// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Scoped reads of original Template and Broker property and flow declarations.

use p4kx_extraction::{
    Archive, Build, CancelToken, DataForge, DataForgeEntry, Record, RecordTree, SourceInventory,
};

use super::unlocalized_record_query::plan::{Owner, Plan};
use super::{CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_origins::CatalogOrigins;
use crate::catalog_query_session::{CatalogQuerySession, ParsedCatalogQueries, checkpoint};
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::DatasetAvailability;
use crate::catalog_template_modules::template_prepared_owner::PreparedOwner;
use crate::guid_text::{CatalogGuid, sort_by_guid};
use crate::mission_flow_owner::FlowOwnerKind;
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;

pub use crate::mission_flow_owner::{MissionFlowOwnerRef, PhysicalFlowOwner};
pub use crate::source_value_catalog::{SourceField, SourceNode, SourcePayload, SourceValueRef};
pub use crate::template_configuration::{
    FlowSelection, MissionFlowConfiguration, MissionPropertyConfiguration, PropertySelection,
};
pub use crate::template_source::{
    OriginalFieldPosition, TemplateSource, TemplateSourceRoot, TemplateSourceValue,
};
pub use crate::template_source_refs::{DeclarationBinding, SourceFieldRef};

/// The original owner family of a mission property declaration.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum MissionPropertyOwnerFamily {
    /// Original `ContractTemplate` Records.
    ContractTemplates,
    /// Original `MissionBrokerEntry` Records.
    MissionBrokerEntries,
}

impl MissionPropertyOwnerFamily {
    const fn kind(self) -> FlowOwnerKind {
        match self {
            Self::ContractTemplates => FlowOwnerKind::ContractTemplate,
            Self::MissionBrokerEntries => FlowOwnerKind::MissionBrokerEntry,
        }
    }
}

/// The owner family and physical Record selection of a mission property query.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MissionPropertySelection {
    /// The original configuration owner family.
    pub family: MissionPropertyOwnerFamily,
    /// The selected Records within the family.
    pub records: CatalogRecordSelection,
}

impl MissionPropertySelection {
    /// Selects physical Records within one original owner family.
    #[must_use]
    pub const fn new(family: MissionPropertyOwnerFamily, records: CatalogRecordSelection) -> Self {
        Self { family, records }
    }
}

/// The original source arena referenced by an owner's property and flow rows.
#[derive(Debug)]
#[non_exhaustive]
pub struct MissionPropertySource<'source> {
    /// The exact original Record that owns the arena.
    pub owner: MissionFlowOwnerRef,
    /// The canonical original source arena of the definition.
    pub arena: TemplateSource<'source>,
}

/// Canonical property and flow rows and their original source closure.
#[derive(Debug)]
#[non_exhaustive]
#[must_use]
pub struct MissionPropertySnapshot<'source> {
    /// The installed Archive Build, when available.
    pub build: Option<Build>,
    /// Availability of the original configuration source families.
    pub availability: DatasetAvailability,
    /// Selected rows in canonical owner GUID and physical declaration order.
    pub rows: Vec<MissionPropertyConfiguration>,
    /// Selected flow rows in canonical owner GUID and physical declaration order.
    pub flows: Vec<MissionFlowConfiguration>,
    /// The selected owners' canonical source arenas in matching order.
    pub sources: Vec<MissionPropertySource<'source>>,
    /// Original DataForge inputs and the uses backing the returned values.
    pub source_inventory: SourceInventory,
    /// Selected Record and field coverage.
    pub coverage: CatalogRecordCoverage,
}

/// Supplies canonical mission property and flow declarations with each owner's original arena.
/// Publish consumer-owned changes after this function returns successfully.
///
/// # Errors
/// Returns input, source evidence, Archive change or cancellation errors.
pub fn with_mission_properties<T>(
    archive: &Archive,
    selection: MissionPropertySelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionPropertySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_mission_properties(selection, cancel, inspect)
}

/// Supplies mission property declarations using a decoded Entry of the same Archive.
///
/// # Errors
/// Returns query errors or an origin mismatch for a foreign Entry.
pub fn with_mission_properties_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: MissionPropertySelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(MissionPropertySnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_mission_properties(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies canonical mission properties through the retained Archive session.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_properties`].
    pub fn with_mission_properties<T>(
        &self,
        selection: MissionPropertySelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionPropertySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_mission_properties(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies canonical mission properties through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns the errors of [`with_mission_properties`].
    pub fn with_mission_properties<T>(
        &self,
        selection: MissionPropertySelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(MissionPropertySnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            Ok(inspect(read(archive, entry, forge, selection, cancel)?))
        })
    }
}

fn read<'source>(
    archive: &Archive,
    entry: &DataForgeEntry,
    forge: &'source DataForge<'source>,
    selection: MissionPropertySelection,
    cancel: Option<&CancelToken>,
) -> Result<MissionPropertySnapshot<'source>, CatalogError> {
    let family = selection.family.kind();
    let Plan {
        mut owners,
        mut coverage,
    } = Plan::read_rule(
        forge,
        family.catalog_kind().records(),
        selection.records,
        cancel,
        None,
    )?;
    sort_by_guid(&mut owners, |owner| CatalogGuid(owner.record.guid()));
    let origins = CatalogOrigins::for_archive_records(archive, entry)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut sources = Vec::with_capacity(owners.len());
    let mut rows = Vec::new();
    let mut flows = Vec::new();
    for Owner { record, tree } in owners {
        checkpoint(cancel)?;
        let scoped = read_owner(&input, family, &record, &tree, cancel)?;
        if !scoped.complete {
            coverage.incomplete_fields.push(record.index());
        }
        rows.extend(scoped.rows);
        flows.extend(scoped.flows);
        sources.push(scoped.source);
    }
    Ok(MissionPropertySnapshot {
        build: archive.build().cloned(),
        availability: DatasetAvailability::from_source_declarations(forge, |name| {
            matches!(name, "ContractTemplate" | "MissionBrokerEntry")
        }),
        rows,
        flows,
        sources,
        source_inventory: origins.uses.finish_with(|| checkpoint(cancel))?,
        coverage,
    })
}

struct OwnerRows<'source> {
    rows: Vec<MissionPropertyConfiguration>,
    flows: Vec<MissionFlowConfiguration>,
    source: MissionPropertySource<'source>,
    complete: bool,
}

fn read_owner<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    family: FlowOwnerKind,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
    cancel: Option<&CancelToken>,
) -> Result<OwnerRows<'source>, CatalogError> {
    let scope = input.record_scope(record, tree)?;
    let fields = OriginalFlowFields::read(&scope, cancel)?;
    let admission = RecordObservation::capture(input, record)?;
    let capture = TemplateFieldCapture::new(&fields, input, &admission)?;
    let PreparedOwner {
        plan,
        members,
        mut references,
    } = PreparedOwner::read(&fields, &capture, family)?;
    let (rows, _) =
        crate::template_property_configuration_read::read(&members, &plan.objectives, family)?;
    let (flows, _) = crate::template_flow_configuration_read::read(&members, &plan, family)?;
    let (arena, failure) =
        crate::template_meanings::read(&members, &plan.flow, references.get_mut(), None, &capture)?;
    let complete = failure.is_none() && arena.values_are_readable();
    Ok(OwnerRows {
        rows,
        flows,
        source: MissionPropertySource {
            owner: MissionFlowOwnerRef::read(&fields, family)?,
            arena,
        },
        complete,
    })
}
