// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

use p4kx_assembly::{LoadoutRecordReader, PortDefinitionPosition};
use p4kx_extraction::{CancelToken, RecordTree};

use super::AlignmentLookup;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_item_alignment::{
    AlignmentOptions, AttachmentDefinitionLink, ItemAttachmentComponent, ItemPortContainer,
    PortDefinitionLink, ScopedTopologyAlignment,
};
use crate::record_observation::RecordObservation;

use crate::{CatalogError, ExportAccountingError};

#[path = "loadout_alignment_read.rs"]
mod read;
use read::DemandReader;

/// Canonical configuration family reached by a resolved Loadout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AlignmentRole {
    /// The definition of an original port used by the resolver.
    Port,
    /// The attachment constraints of a resolved Entity.
    Attachment,
}

/// What the exact reached Record and configuration role established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AlignmentDependencyState {
    /// The consulted role was completely classified and projected.
    Complete,
    /// The Catalog does not admit this declaration as a configuration owner.
    NotAdmitted,
    /// The requested physical Record is missing.
    MissingRecord,
    /// Header or owner-selection evidence could not be read.
    UnreadableHeader,
    /// The reached configuration's Record body could not be decoded.
    UnreadableBody,
    /// The role has incomplete original selection or field evidence.
    IncompleteProjection,
}

/// Physical dependency visited for a Loadout configuration link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct AlignmentDependency {
    /// The reached physical Record index in the queried DataForge.
    pub record_index: u32,
    /// The configuration role consulted for this Record.
    pub role: AlignmentRole,
    /// The outcome of the scoped read.
    pub state: AlignmentDependencyState,
}

enum Rows<'source> {
    Ports(Vec<ItemPortContainer<'source>>),
    Attachments(Vec<ItemAttachmentComponent<'source>>),
}

struct Prepared<'source> {
    state: AlignmentDependencyState,
    rows: Option<Rows<'source>>,
}

impl<'source> Prepared<'source> {
    const fn unavailable(state: AlignmentDependencyState) -> Self {
        Self { state, rows: None }
    }

    fn read<T>(read: ScopedSourceRead<T>, wrap: impl FnOnce(T) -> Rows<'source>) -> Self {
        Self {
            state: AlignmentDependencyState::from_scoped_read(&read),
            rows: Some(wrap(read.value)),
        }
    }
}

impl AlignmentDependencyState {
    pub(crate) fn from_scoped_read<T>(read: &ScopedSourceRead<T>) -> Self {
        if read.selection_complete
            && !read
                .notes
                .iter()
                .any(|note| note.description().issue.blocks_projection())
        {
            Self::Complete
        } else {
            Self::IncompleteProjection
        }
    }
}

/// Resolves Loadout configuration links from reached physical Records and roles.
pub(crate) struct DirectAlignment<'source> {
    reader: DemandReader<'source>,
    rows: BTreeMap<(u32, AlignmentRole), Prepared<'source>>,
}

impl<'source> DirectAlignment<'source> {
    /// Starts an empty original dependency scope on the query projection.
    pub(crate) fn new(
        projection: &'source CatalogProjection<'source, 'source>,
        cancel: Option<&'source CancelToken>,
    ) -> Self {
        Self {
            reader: DemandReader::new(projection, cancel),
            rows: BTreeMap::new(),
        }
    }

    /// Shares immutable reads and enum options across owner projections.
    pub(crate) fn with_shared_inputs(
        mut self,
        options: &'source AlignmentOptions<'source>,
        records: &'source dyn LoadoutRecordReader,
    ) -> Self {
        self.reader = self.reader.with_shared_inputs(options, records);
        self
    }

    /// Returns the original dependencies reached in physical order.
    pub(crate) fn dependencies(&self) -> impl Iterator<Item = AlignmentDependency> + '_ {
        self.rows
            .iter()
            .map(|(&(record_index, role), prepared)| AlignmentDependency {
                record_index,
                role,
                state: prepared.state,
            })
    }

    pub(crate) fn read_configuration<T>(
        &mut self,
        record: u32,
        read: impl FnOnce(
            &RecordTree<'source>,
            &CatalogProjection<'_, 'source>,
            &RecordObservation<'_, 'source>,
            Option<&CancelToken>,
        ) -> Result<T, CatalogError>,
    ) -> Result<Result<T, AlignmentDependencyState>, CatalogError> {
        crate::catalog_query_session::checkpoint(self.reader.cancel)?;
        self.reader.read_configuration(record, read)
    }

    pub(crate) fn include_port_inventory(&mut self, record: u32) -> Result<(), CatalogError> {
        self.reach(record, AlignmentRole::Port)?;
        Ok(())
    }

    pub(crate) fn into_topology(self) -> ScopedTopologyAlignment<'source> {
        let mut facts = ScopedTopologyAlignment::default();
        for prepared in self.rows.into_values() {
            match prepared.rows {
                Some(Rows::Ports(rows)) => facts.append_ports(rows),
                Some(Rows::Attachments(rows)) => facts.append_attachments(rows),
                None => {}
            }
        }
        facts
    }

    fn reach(
        &mut self,
        record: u32,
        role: AlignmentRole,
    ) -> Result<&Prepared<'source>, CatalogError> {
        crate::catalog_query_session::checkpoint(self.reader.cancel)?;
        match self.rows.entry((record, role)) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => Ok(entry.insert(self.reader.read(record, role)?)),
        }
    }
}

impl AlignmentLookup for DirectAlignment<'_> {
    fn port(
        &mut self,
        position: PortDefinitionPosition,
    ) -> Result<PortDefinitionLink, CatalogError> {
        let dependency = self.reach(position.record_index(), AlignmentRole::Port)?;
        match &dependency.rows {
            Some(Rows::Ports(rows)) => PortDefinitionLink::from_original(rows, position),
            None => Ok(PortDefinitionLink::NotProjected),
            Some(Rows::Attachments(_)) => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }

    fn attachment(
        &mut self,
        record: u32,
        component: Option<usize>,
    ) -> Result<AttachmentDefinitionLink, CatalogError> {
        let Some(component) = component else {
            return Ok(AttachmentDefinitionLink::NotDeclared);
        };
        let dependency = self.reach(record, AlignmentRole::Attachment)?;
        match &dependency.rows {
            Some(Rows::Attachments(rows)) => {
                AttachmentDefinitionLink::from_original(rows, record, component)
            }
            None => Ok(AttachmentDefinitionLink::NotProjected),
            Some(Rows::Ports(_)) => Err(ExportAccountingError::ConflictingSourceDatum.into()),
        }
    }
}
