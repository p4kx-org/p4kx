// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ENTITY_TYPE, LoadoutRecordReader};
use p4kx_extraction::{CancelToken, DataForgeError, Record, RecordTree, TreeBuilder};

use super::{AlignmentDependencyState, AlignmentRole, Prepared, Rows};
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::catalog_entity_configurations;
use crate::catalog_v3::catalog_item_alignment::AlignmentOptions;
use crate::entity_selection::{self, Selected};
use crate::record_observation::RecordObservation;
use crate::scan::{CRAFTER_FIELD, CRAFTER_STRUCT};
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) struct DemandReader<'source> {
    projection: &'source CatalogProjection<'source, 'source>,
    pub(super) cancel: Option<&'source CancelToken>,
    trees: TreeBuilder<'source>,
    options: Option<AlignmentOptions<'source>>,
    shared_options: Option<&'source AlignmentOptions<'source>>,
    records: Option<&'source dyn LoadoutRecordReader>,
}

impl<'source> DemandReader<'source> {
    pub(super) fn new(
        projection: &'source CatalogProjection<'source, 'source>,
        cancel: Option<&'source CancelToken>,
    ) -> Self {
        Self {
            projection,
            cancel,
            trees: TreeBuilder::new(),
            options: None,
            shared_options: None,
            records: None,
        }
    }

    pub(super) fn with_shared_inputs(
        mut self,
        options: &'source AlignmentOptions<'source>,
        records: &'source dyn LoadoutRecordReader,
    ) -> Self {
        self.shared_options = Some(options);
        self.records = Some(records);
        self
    }

    pub(super) fn read(
        &mut self,
        index: u32,
        role: AlignmentRole,
    ) -> Result<Prepared<'source>, CatalogError> {
        let (record, tree) = match self.read_owner(index)? {
            Ok(owner) => owner,
            Err(state) => return Ok(Prepared::unavailable(state)),
        };
        let projection = self.projection;
        let admission = RecordObservation::capture(projection.original(), &record)?;
        let options = if let Some(options) = self.shared_options {
            options
        } else {
            if self.options.is_none() {
                self.options = Some(AlignmentOptions::read(
                    projection.original().forge(),
                    self.cancel,
                )?);
            }
            self.options
                .as_ref()
                .ok_or(ExportAccountingError::ConflictingSourceDatum)?
        };
        let cancel = self.cancel;
        match role {
            AlignmentRole::Port => catalog_entity_configurations::read_item_ports_scoped(
                tree.as_ref(),
                projection,
                &admission,
                options,
                cancel,
            )
            .map(|read| Prepared::read(read, Rows::Ports)),
            AlignmentRole::Attachment => {
                catalog_entity_configurations::read_attachment_constraints(
                    tree.as_ref(),
                    projection,
                    &admission,
                    options,
                    cancel,
                )
                .map(|read| Prepared::read(read, Rows::Attachments))
            }
        }
    }

    pub(super) fn read_configuration<T>(
        &mut self,
        index: u32,
        read: impl FnOnce(
            &RecordTree<'source>,
            &CatalogProjection<'_, 'source>,
            &RecordObservation<'_, 'source>,
            Option<&CancelToken>,
        ) -> Result<T, CatalogError>,
    ) -> Result<Result<T, AlignmentDependencyState>, CatalogError> {
        let (record, tree) = match self.read_owner(index)? {
            Ok(owner) => owner,
            Err(state) => return Ok(Err(state)),
        };
        let admission = RecordObservation::capture(self.projection.original(), &record)?;
        read(tree.as_ref(), self.projection, &admission, self.cancel).map(Ok)
    }

    fn read_owner(
        &mut self,
        index: u32,
    ) -> Result<Result<(Record<'source>, ReadTree<'source>), AlignmentDependencyState>, CatalogError>
    {
        let Some(record) = self.projection.original().forge().record(index) else {
            return Ok(Err(AlignmentDependencyState::MissingRecord));
        };
        match self.admitted(&record) {
            Ok(true) => {}
            Ok(false) => return Ok(Err(AlignmentDependencyState::NotAdmitted)),
            Err(DataForgeError::ObservationStopped) => return Err(CatalogError::Cancelled),
            Err(_) => return Ok(Err(AlignmentDependencyState::UnreadableHeader)),
        }
        match self.read_tree(&record) {
            Ok(tree) => Ok(Ok((record, tree))),
            Err(DataForgeError::ObservationStopped) => Err(CatalogError::Cancelled),
            Err(_) => Ok(Err(AlignmentDependencyState::UnreadableBody)),
        }
    }

    fn read_tree(&mut self, record: &Record<'source>) -> Result<ReadTree<'source>, DataForgeError> {
        if let Some(cache) = self.records
            && let Some(tree) = cache.read(record)?
        {
            return Ok(ReadTree::Shared(tree));
        }
        self.trees.build(record).map(ReadTree::Owned)
    }

    fn admitted(&mut self, record: &Record<'source>) -> Result<bool, DataForgeError> {
        if record.type_name()? != ENTITY_TYPE {
            return Ok(false);
        }
        match entity_selection::select(record, None)? {
            Some(Selected::Entity(Kind::Crafters)) => {
                let forge = self.projection.original().forge();
                forge
                    .pointer_probe(CRAFTER_FIELD, CRAFTER_STRUCT)
                    .as_ref()
                    .map_or(Ok(false), |probe| self.trees.holds(probe, record))
            }
            Some(Selected::Entity(_)) => Ok(true),
            Some(Selected::Leftover(_)) | None => Ok(false),
        }
    }
}

enum ReadTree<'source> {
    Owned(RecordTree<'source>),
    Shared(Arc<RecordTree<'source>>),
}

impl<'source> AsRef<RecordTree<'source>> for ReadTree<'source> {
    fn as_ref(&self) -> &RecordTree<'source> {
        match self {
            Self::Owned(tree) => tree,
            Self::Shared(tree) => tree.as_ref(),
        }
    }
}
