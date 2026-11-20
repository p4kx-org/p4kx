// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Mutex;

use p4kx_extraction::{CancelToken, Filter, Record, RecordTree};

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValueFact};
use crate::catalog_v3::ammo_definition::AmmoSelection;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_ammo_bindings::AmmoBinding;
use crate::catalog_v3::catalog_ammo_containers::AmmoContainer;
use crate::catalog_v3::catalog_item_components::ItemComponentRows;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::entity_selection::{self, Selected};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_evidence_lock;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::record_observation::RecordObservation;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, ExportAccountingError, Kind};

/// The canonical component and binding needed by a selected weapon's linked magazine.
pub(crate) struct MagazineDependency<'source> {
    pub(crate) container: AmmoContainer<'source>,
    pub(crate) binding: AmmoBinding<'source>,
    numbers: Vec<CapturedNumber>,
}

impl<'source> MagazineDependency<'source> {
    pub(crate) fn collect(self, components: &mut ItemComponentRows<'source>) {
        components.ammo_containers.push(self.container);
        components.ammo_bindings.push(self.binding);
        components.numbers.extend(self.numbers);
    }
}

/// Retains one canonical container per out-of-selection physical magazine slot.
pub(crate) struct MagazineDependencies<'options, 'source> {
    filter: Option<&'options Filter>,
    state: Mutex<DependencyState<'source>>,
}

#[derive(Default)]
struct DependencyState<'source> {
    rows: BTreeMap<(u32, u32), MagazineDependency<'source>>,
    ledger: ExportLedger,
}

impl<'options, 'source> MagazineDependencies<'options, 'source> {
    pub(crate) fn new(filter: Option<&'options Filter>) -> Self {
        Self {
            filter,
            state: Mutex::new(DependencyState::default()),
        }
    }

    pub(super) fn retain<'scope, 'read>(
        &self,
        input: &OriginalCatalogInput<'_, 'source>,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
        fields: &OriginalFlowFields<'scope, 'read, 'source>,
        container: &OriginalValueFact<'scope, 'read, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), CatalogError> {
        let ordinary =
            entity_selection::select(record, self.filter).map_err(CatalogError::DataForge)?;
        if ordinary == Some(Selected::Entity(Kind::Items)) {
            return Ok(());
        }

        let context = input.record_context(record)?;
        let owner = ComponentOwner::read(tree, &context, fields, container)?;
        let (Some(index), Some(ordinal)) = owner.key() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let mut state = export_evidence_lock::lock(&self.state)?;
        let DependencyState { rows, ledger } = &mut *state;
        let std::collections::btree_map::Entry::Vacant(row) = rows.entry((index, ordinal)) else {
            return Ok(());
        };
        let admission = RecordObservation::capture(input, record)?;
        let capture = TemplateFieldCapture::new(fields, input, &admission)?;
        let numbers = RefCell::new(Vec::new());
        let observed = Observed::new(fields, input, container, &capture, &numbers);
        let mut notes = Vec::new();
        let binding = AmmoBinding::container(owner.clone(), observed, &mut notes)?;
        let container = AmmoContainer::read_with_selection(
            owner,
            observed,
            &mut notes,
            AmmoSelection::Dependency,
        )?;
        let kind = Kind::Items;
        let role = ExportSourceRole::Dependency;
        ledger.merge(crate::export_field_walk::inspect_observed(
            Some(input),
            tree,
            &admission,
            Some(kind),
            role,
            cancel,
        )?);
        crate::source_projection_notes::record(kind, role, &admission, tree, notes, ledger)?;
        ledger.observe(admission.source(Some(kind), role, SourceDisposition::Projected)?);
        row.insert(MagazineDependency {
            container,
            binding,
            numbers: numbers.into_inner(),
        });
        drop(state);
        Ok(())
    }

    pub(crate) fn take(
        &self,
        ledger: &mut ExportLedger,
    ) -> Result<impl Iterator<Item = MagazineDependency<'source>>, ExportAccountingError> {
        let mut state = export_evidence_lock::lock(&self.state)?;
        ledger.merge(std::mem::take(&mut state.ledger));
        Ok(std::mem::take(&mut state.rows).into_values())
    }
}
