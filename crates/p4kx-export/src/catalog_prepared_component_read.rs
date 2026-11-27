// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::OnceCell;

use p4kx_extraction::{CancelToken, DataForge, PointerProbe};

use super::{
    COMPONENTS, ComponentKind, ComponentPlan, ComponentRead, ComponentSelection, Projection,
    read_in,
};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;

pub(in super::super) struct PreparedComponent<'source> {
    forge: &'source DataForge<'source>,
    kind: ComponentKind,
    strict_probe: OnceCell<Option<PointerProbe<'source>>>,
    export_probe: OnceCell<Option<PointerProbe<'source>>>,
}

impl<'source> PreparedComponent<'source> {
    pub(in super::super) const fn new(
        forge: &'source DataForge<'source>,
        kind: ComponentKind,
    ) -> Self {
        Self {
            forge,
            kind,
            strict_probe: OnceCell::new(),
            export_probe: OnceCell::new(),
        }
    }

    pub(super) fn read<Row>(
        &self,
        input: &OriginalCatalogInput<'_, 'source>,
        selection: &ComponentSelection<'_>,
        cancel: Option<&CancelToken>,
        project: Projection<'source, Row>,
        coverage: Option<&mut CatalogRecordCoverage>,
    ) -> Result<ComponentRead<'source, Row>, CatalogError> {
        input.check_forge(self.forge)?;
        let probe = if selection.unclassified_role.is_some() {
            self.strict_probe.get_or_init(|| {
                self.forge
                    .strict_pointer_probe(COMPONENTS, self.kind.source_type)
            })
        } else {
            self.export_probe
                .get_or_init(|| self.forge.pointer_probe(COMPONENTS, self.kind.source_type))
        };
        read_in(
            self.forge,
            input,
            selection,
            cancel,
            ComponentPlan {
                kind: &self.kind,
                probe: probe.as_ref(),
            },
            project,
            coverage,
        )
    }

    pub(super) fn read_selected<Row>(
        &self,
        input: &OriginalCatalogInput<'_, 'source>,
        physical: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        project: Projection<'source, Row>,
    ) -> Result<(Vec<Row>, CatalogRecordCoverage), CatalogError> {
        checkpoint(cancel)?;
        let selection = ComponentSelection {
            physical,
            ..ComponentSelection::discovery()
        };
        let mut coverage = CatalogRecordCoverage::default();
        let read = self.read(input, &selection, cancel, project, Some(&mut coverage))?;
        Ok((read.rows, coverage))
    }

    /// Reads strict physical coverage and preserves any canonical partial rows.
    pub(in super::super) fn read_with_partials<Row>(
        &self,
        input: &OriginalCatalogInput<'_, 'source>,
        record_index: u32,
        cancel: Option<&CancelToken>,
        project: Projection<'source, Row>,
    ) -> Result<(Vec<Row>, CatalogRecordCoverage), CatalogError> {
        let physical = CatalogRecordSelection::RecordIndex(record_index);
        let (rows, coverage) = self.read_selected(input, physical, cancel, project)?;
        if !rows.is_empty() || coverage.unreadable_bodies.is_empty() {
            return Ok((rows, coverage));
        }
        checkpoint(cancel)?;
        let selection = ComponentSelection {
            physical,
            ..ComponentSelection::export(None, None)
        };
        let canonical = self.read(input, &selection, cancel, project, None)?;
        Ok((canonical.rows, coverage))
    }
}
