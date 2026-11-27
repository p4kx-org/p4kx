// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken};

use super::super::ShopRowCoverage;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::EntryCapture;
use crate::catalog_table::DatasetAvailability;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_inventories::Inventories;
use crate::shop_inventory_resolution::InventoryResolver;
use crate::shop_registry::RegistryDocument;
use crate::shop_summary::ShopSummary;
use crate::shops::resolution_scope::{SelectedPlacement, needed_bodies, selected_lists};
use crate::shops::row_query::input::{RegistryInput, read_registry};
use crate::shops::shop_list::{self, PreparedShop};
use crate::shops::{ShopDeclarations, record_registry};
use crate::{CatalogError, Kind, export_health_entries};

pub(super) struct Completed {
    pub(super) availability: DatasetAvailability,
    pub(super) row_count: usize,
    pub(super) coverage: ShopRowCoverage,
}

struct Scope<'input, 'archive> {
    archive: &'input Archive,
    capture: &'input EntryCapture<'archive>,
    cancel: Option<&'input CancelToken>,
    ledger: ExportLedger,
    summary: ShopSummary,
    declarations_complete: Option<bool>,
    inventory_payloads: usize,
}

pub(super) fn visit<E: From<CatalogError>>(
    archive: &Archive,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
    emit: impl FnMut(PreparedShop) -> Result<(), E>,
) -> Result<Completed, E> {
    let mut scope = Scope {
        archive,
        capture,
        cancel,
        ledger: ExportLedger::default(),
        summary: ShopSummary::default(),
        declarations_complete: None,
        inventory_payloads: 0,
    };
    scope.ledger.inventory(Kind::Shops);
    let RegistryInput {
        document,
        availability,
        directory_complete,
    } = read_registry(archive, capture, &mut scope.summary, &mut scope.ledger)?;
    let (row_count, registry_complete) = match document {
        Some(document) => scope.visit_document(document, emit)?,
        None => (0, matches!(availability, DatasetAvailability::Absent)),
    };
    let entries =
        export_health_entries::compile(scope.ledger.entries).map_err(CatalogError::from)?;
    Ok(Completed {
        availability,
        row_count,
        coverage: ShopRowCoverage {
            registry_complete: directory_complete && registry_complete,
            declarations_complete: scope.declarations_complete,
            inventory_payloads: scope.inventory_payloads,
            names: None,
            entries: export_health_entries::counts(Kind::Shops, &entries.sources)
                .map_err(CatalogError::from)?,
        },
    })
}

impl Scope<'_, '_> {
    fn visit_document<E: From<CatalogError>>(
        &mut self,
        document: RegistryDocument,
        emit: impl FnMut(PreparedShop) -> Result<(), E>,
    ) -> Result<(usize, bool), E> {
        let RegistryDocument {
            rows,
            source,
            complete,
        } = document;
        let mut admitted = Vec::new();
        let disposition =
            shop_list::visit_registry(&rows, &mut self.summary, |raw, placement, _| {
                checkpoint(self.cancel)?;
                admitted
                    .try_reserve(1)
                    .map_err(CatalogError::SourceUseAllocation)?;
                admitted.push((raw, placement));
                Ok(())
            })?;
        record_registry(source, disposition, &mut self.ledger);
        admitted.sort_unstable_by(|left, right| left.1.key.cmp(&right.1.key));
        let row_count = admitted.len();
        if !admitted.is_empty() {
            self.visit_admitted(admitted, emit)?;
        }
        Ok((
            row_count,
            complete && disposition == SourceDisposition::Projected,
        ))
    }

    fn visit_admitted<E: From<CatalogError>>(
        &mut self,
        admitted: Vec<SelectedPlacement<'_>>,
        mut emit: impl FnMut(PreparedShop) -> Result<(), E>,
    ) -> Result<(), E> {
        let declarations =
            ShopDeclarations::read(self.archive, self.cancel, self.capture, &mut self.summary)?;
        let complete = declarations.complete();
        let ShopDeclarations {
            containers,
            prefabs,
            ..
        } = declarations;
        let needed = needed_bodies(&containers, &admitted);
        let (mut inventories, payloads) = Inventories::prepare_shops(
            self.archive,
            |stem| needed.contains(stem),
            self.capture,
            &mut self.ledger,
            self.cancel,
        )?;
        let lists = selected_lists(
            &containers,
            &mut inventories,
            &mut self.ledger,
            |_, stem| needed.contains(stem),
        )?;
        let resolver = InventoryResolver::new(
            &containers,
            &inventories,
            &lists,
            &prefabs,
            self.capture,
            complete,
        );
        for (raw, placement) in admitted {
            checkpoint(self.cancel)?;
            let row = resolver.row(raw, &placement.key, &placement.entity_guid)?;
            let cells = resolver.compatibility(&row, &placement.entity_guid)?;
            emit(shop_list::prepare(
                raw,
                placement,
                &containers,
                cells,
                self.capture.source_uses(),
            )?)?;
        }
        self.declarations_complete = Some(complete);
        self.inventory_payloads = payloads;
        self.ledger.merge(containers.ledger);
        self.ledger.merge(prefabs.ledger);
        Ok(())
    }
}
