// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{self, ArchiveFiles, LoadoutError, PaintTagIndex};
use p4kx_extraction::{
    Archive, CancelToken, DataForgeError, FieldOrigin, Record, RecordTree, TreeBuilder,
};

use super::super::EquipmentDependencyCoverage;
use super::{owners, record_inputs};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::CatalogRecordCoverage;
use crate::catalog_query_session::checkpoint;
use crate::catalog_read::original::OriginalFlowFields;
use crate::export_ledger::ExportLedger;
use crate::guid_text::CatalogGuid;
use crate::loadout::ledger::Capture;
use crate::paint::{PaintOwner, PaintRequirements};
use crate::paint_port::VehiclePorts;
use crate::record_header_field::RecordHeaderField;
use crate::{CatalogError, Kind, ship};

pub(super) struct Fleet {
    pub(super) matches: Vec<PaintOwner>,
    pub(super) records: CatalogRecordCoverage,
    pub(super) dependencies: EquipmentDependencyCoverage,
    pub(super) origins: Vec<FieldOrigin>,
}

pub(super) fn read<'source>(
    archive: &'source Archive,
    input: &'source OriginalCatalogInput<'source, 'source>,
    cancel: Option<&CancelToken>,
    paint: &PaintRequirements,
    index: &PaintTagIndex<'_>,
) -> Result<Fleet, CatalogError> {
    let forge = input.forge();
    let files = ArchiveFiles::new(archive);

    let mut fleet = Fleet {
        matches: Vec::new(),
        records: CatalogRecordCoverage::default(),
        dependencies: EquipmentDependencyCoverage::default(),
        origins: Vec::new(),
    };
    let context = ShipContext {
        input,
        files: &files,
        cancel,
        paint,
        index,
    };
    let mut trees = TreeBuilder::new();
    let mut ledger = ExportLedger::default();
    for record in forge.records() {
        checkpoint(cancel)?;
        if record
            .record_path()
            .is_ok_and(|path| !Kind::Ships.records().matches_path(path))
        {
            continue;
        }
        let (selected, header) = owners::admitted(input, &record, Kind::Ships, &mut fleet.records)?;
        fleet.origins.push(header);
        if !selected {
            continue;
        }
        let capture = Arc::new(Capture::for_input(
            input,
            &ExportLedger::default(),
            cancel,
            Kind::Loadouts,
        ));
        trees.set_read_observer(Some(capture.clone()));
        let tree = trees.build(&record);
        trees.set_read_observer(None);
        let tree = match tree {
            Ok(tree) => tree,
            Err(DataForgeError::ObservationStopped) => {
                capture.finish()?;
                return Err(CatalogError::Cancelled);
            }
            Err(_) => {
                ledger.merge(capture.finish()?);
                fleet.records.unreadable_bodies.push(record.index());
                continue;
            }
        };
        fleet.project(&context, &record, &tree, &capture, &mut ledger)?;
    }
    fleet.dependencies.finish(archive, &ledger)?;
    Ok(fleet)
}

struct ShipContext<'context, 'source> {
    input: &'source OriginalCatalogInput<'source, 'source>,
    files: &'context ArchiveFiles<'source>,

    cancel: Option<&'context CancelToken>,
    paint: &'context PaintRequirements,
    index: &'context PaintTagIndex<'context>,
}

impl Fleet {
    fn project<'source>(
        &mut self,
        context: &ShipContext<'_, 'source>,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
        capture: &Arc<Capture<'source>>,
        ledger: &mut ExportLedger,
    ) -> Result<(), CatalogError> {
        let scope = context.input.record_scope(record, tree)?;
        let fields = OriginalFlowFields::read(&scope, context.cancel)?;
        let body = record_inputs::observe(tree, &fields)?;
        let origins = context.input.origins();
        let header = origins.record(record.index());
        let guid = header.observe_header(
            RecordHeaderField::Guid,
            p4kx_extraction::SourceUseRole::Payload,
        )?;
        self.origins.push(
            origins
                .uses
                .origin_with_origins([&body.origin, &origins.uses.origin(&[guid], true)], true)?,
        );
        if !body.origin.source_set_complete() {
            self.records.incomplete_fields.push(record.index());
        }
        let mut ports = ship::paint_ports(tree);
        if body.has_vehicle {
            let (effective, proof) =
                context.vehicle(record, tree, capture, &mut self.dependencies)?;
            ports.extend(effective.ports);
            self.origins.extend(proof);
        }
        ledger.merge(capture.finish()?);
        if !ports.is_empty()
            && context
                .paint
                .accepts(&ports, &ship::lowered_tags(tree), context.index)
        {
            let index = header.observe_header(
                RecordHeaderField::RecordIndex,
                p4kx_extraction::SourceUseRole::Payload,
            )?;
            self.origins.push(origins.uses.origin(&[index], true));
            self.matches.push(PaintOwner {
                record_index: record.index(),
                guid: CatalogGuid(record.guid()),
            });
        }
        Ok(())
    }
}

impl<'source> ShipContext<'_, 'source> {
    fn vehicle(
        &self,
        record: &Record<'source>,
        tree: &RecordTree<'source>,
        capture: &Arc<Capture<'source>>,
        coverage: &mut EquipmentDependencyCoverage,
    ) -> Result<(VehiclePorts, Option<FieldOrigin>), CatalogError> {
        let read = p4kx_assembly::read_observed_vehicle_tree(
            self.input.forge(),
            record,
            tree,
            self.files,
            capture.clone(),
        );
        let result = match read {
            Ok(Some(implementation)) => {
                let ports = VehiclePorts::of(Some(&implementation), None, &[]);
                let proof = capture.paint_vehicle_survey(
                    record.index(),
                    &implementation,
                    self.files.archive(),
                )?;
                if ports.unreadable.is_some() || !proof.source_set_complete() {
                    coverage.source_incomplete(record.index());
                }
                (ports, Some(proof))
            }
            Ok(None) => {
                coverage.source_incomplete(record.index());
                (VehiclePorts::default(), None)
            }
            Err(LoadoutError::MissingVehicleImplementation { path }) => {
                checkpoint(self.cancel)?;
                let proof = capture.paint_missing_vehicle_survey(
                    record.index(),
                    &path,
                    self.files.archive(),
                )?;
                if !proof.source_set_complete() {
                    coverage.source_incomplete(record.index());
                }
                (VehiclePorts::default(), Some(proof))
            }
            Err(LoadoutError::ObservationStopped) => {
                capture.finish()?;
                return Err(CatalogError::Cancelled);
            }
            Err(_) => {
                checkpoint(self.cancel)?;
                coverage.record_failure(record.index());
                (VehiclePorts::default(), None)
            }
        };
        Ok(result)
    }
}
