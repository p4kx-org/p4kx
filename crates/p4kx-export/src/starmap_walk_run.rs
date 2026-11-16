// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use p4kx_extraction::{Archive, CancelToken, SourceValue};

use super::{ContainerRows, System, SystemRead, WalkPath};
use crate::catalog_read::original::EntryCapture;
use crate::entry_source::EntrySource;
use crate::starmap_containers::Limits;
use crate::starmap_containers::jump_class::JumpEndpointRecords;
use crate::starmap_index::Index;
use crate::starmap_parse::Body;
use crate::{CatalogError, ExportAccountingError, HealthText};

impl<'a> System<'a> {
    #[cfg(test)]
    pub(crate) fn run(
        index: &'a Index<'a>,
        bodies: &'a HashMap<usize, Body>,
        name: &Arc<str>,
        entry: usize,
        cancel: Option<&'a CancelToken>,
        limits: Limits,
        (archive, capture, endpoints): (
            &'a Archive,
            &'a EntryCapture<'a>,
            Option<&'a JumpEndpointRecords>,
        ),
    ) -> Result<SystemRead, CatalogError> {
        Self::run_with_output(
            index,
            bodies,
            name,
            entry,
            cancel,
            limits,
            (archive, capture, endpoints, ContainerRows::Typed),
        )
    }

    pub(crate) fn run_with_output(
        index: &'a Index<'a>,
        bodies: &'a HashMap<usize, Body>,
        name: &Arc<str>,
        entry: usize,
        cancel: Option<&'a CancelToken>,
        limits: Limits,
        (archive, capture, endpoints, container_rows): (
            &'a Archive,
            &'a EntryCapture<'a>,
            Option<&'a JumpEndpointRecords>,
            ContainerRows<'a>,
        ),
    ) -> Result<SystemRead, CatalogError> {
        let root = index
            .entries
            .get(entry)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let source = EntrySource::outer(capture, archive, root)?;
        let directory = source.receipt().physical_reference(capture)?;
        let SourceValue::Present(reference) = directory.value() else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let mut system = Self {
            archive,
            index,
            bodies,
            cancel,
            name: Arc::clone(name),
            name_cell: capture.source_uses().value(
                SourceValue::Present(HealthText::new(name)),
                directory.contributions().0,
                true,
            ),
            root: Arc::from(directory.contributions().0),
            root_entry: reference.entry_index,
            capture,
            endpoints,
            read: SystemRead::default(),
            container_rows,
            limits,
            budget: limits.children,
            open: Vec::new(),
            seen: [0; 3],
        };
        system.container(entry, &WalkPath::ROOT)?;
        Ok(system.read)
    }
}
