// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::ptr;
use std::sync::Mutex;

use p4kx_assembly::{
    EntityNode, LoadoutPath, LoadoutPathObserver, LoadoutPathOperation, LoadoutReadEvent,
    LoadoutReadObserver, LoadoutRecordText, LoadoutSelection, PortNode, XmlLoadoutPathSite,
};
use p4kx_extraction::{
    DataForge, EntryOrigin, FieldOrigin, InputScope, Record, RecordTree, SourceCoordinateOmission,
    SourceEvidenceError, SourceInput, SourceLocation, SourceUseRole, StructValue, Value,
};

use crate::forge_origins::{EvidenceError, ForgeOrigins};

#[derive(Debug)]
pub(crate) struct ResolvedLoadout {
    pub(super) tree: EntityNode,
    sources: Sites,
    port_count: usize,
}

impl ResolvedLoadout {
    pub(crate) const fn port_count(&self) -> usize {
        self.port_count
    }

    pub(super) fn port_name(
        &self,
        output: &ForgeOrigins,
        port: &PortNode,
        role: SourceUseRole,
    ) -> Result<FieldOrigin, EvidenceError> {
        let site = selected_name(port).and_then(|selection| self.sources.get(selection));
        Ok(match site {
            Some(Site::Record {
                index,
                ordinal,
                text,
            }) => text.origin(
                &output.uses,
                &output.scope,
                *index,
                *ordinal,
                role,
                output.complete,
            )?,
            Some(Site::Xml { original, location }) => {
                xml_origin(output, original, location.as_ref(), role)?
            }
            None => output.uses.origin(&[], false),
        })
    }
}

type Sites = HashMap<LoadoutSelection, Site>;

#[derive(Debug)]
enum Site {
    Record {
        index: u32,
        ordinal: u64,
        text: LoadoutRecordText,
    },
    Xml {
        original: EntryOrigin,
        location: Option<SourceLocation>,
    },
}

fn selected_name(port: &PortNode) -> Option<&LoadoutSelection> {
    port.name_proof()
        .filter(|path| path.value() == port.name())
        .and_then(LoadoutPath::selection)
        .filter(|selection| selection.operation() == LoadoutPathOperation::PortName)
}

fn xml_origin(
    output: &ForgeOrigins,
    original: &EntryOrigin,
    location: Option<&SourceLocation>,
    role: SourceUseRole,
) -> Result<FieldOrigin, SourceEvidenceError> {
    let scope = output
        .uses
        .observe(SourceInput::Archive(original), InputScope::Payload)?;
    let (site, complete) = match location {
        Some(location) => (
            scope.at(location.clone(), role)?,
            original.ancestors().all(|ancestor| ancestor.is_some()),
        ),
        None => (
            scope.at_opaque(role, SourceCoordinateOmission::Unavailable)?,
            false,
        ),
    };
    Ok(output.uses.origin(&[site], complete))
}

pub(super) struct Capture<'forge> {
    forge: &'forge DataForge<'forge>,
    sites: Mutex<Sites>,
}

impl<'forge> Capture<'forge> {
    pub(super) fn new(forge: &'forge DataForge<'forge>) -> Self {
        Self {
            forge,
            sites: Mutex::default(),
        }
    }

    pub(super) fn finish(&self, tree: EntityNode) -> Result<ResolvedLoadout, EvidenceError> {
        let mut observed = std::mem::take(
            &mut *self
                .sites
                .lock()
                .map_err(|_| SourceEvidenceError::PoisonedEvidence)?,
        );
        let mut sources = Sites::new();
        let mut port_count = 0;
        tree.visit_ports(|_, route| {
            port_count += 1;
            if let Some(selection) = route.last().and_then(|port| selected_name(port))
                && let Some(site) = observed.remove(selection)
            {
                sources.insert(selection.clone(), site);
            }
        });
        Ok(ResolvedLoadout {
            tree,
            sources,
            port_count,
        })
    }
}

impl LoadoutReadObserver for Capture<'_> {
    fn observe(&self, _event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    fn observes_port_names(&self) -> bool {
        true
    }

    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        if !tree.is_from(record) || !record.is_from(self.forge) {
            return parse(None);
        }
        let names = RecordNames {
            forge: self.forge,
            pending: RefCell::default(),
        };
        parse(Some(&names))?;
        let mut pending = names.pending.into_inner();
        if pending.is_empty() {
            return ControlFlow::Continue(());
        }
        let mut found = Vec::new();
        tree.root().visit_owned_values(|visit| {
            if let Some(names) = pending.remove(&ptr::from_ref(visit.value()).addr()) {
                for name in names {
                    found.push((
                        name.selection,
                        Site::Record {
                            index: record.index(),
                            ordinal: visit.ordinal() as u64,
                            text: name.text,
                        },
                    ));
                }
            }
        });
        let Ok(mut sites) = self.sites.lock() else {
            return ControlFlow::Break(());
        };
        sites.extend(found);
        ControlFlow::Continue(())
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        if selection.operation() != LoadoutPathOperation::PortName
            || site.observation().read_outcome().is_err()
        {
            return ControlFlow::Continue(());
        }
        let Some(original) = site.observation().source().entry_origin() else {
            return ControlFlow::Continue(());
        };
        let Ok(mut sites) = self.sites.lock() else {
            return ControlFlow::Break(());
        };
        sites.insert(
            selection.clone(),
            Site::Xml {
                original: original.clone(),
                location: site.location(),
            },
        );
        ControlFlow::Continue(())
    }
}

struct RecordNames<'forge> {
    forge: &'forge DataForge<'forge>,
    pending: RefCell<HashMap<usize, Vec<PendingName>>>,
}

struct PendingName {
    selection: LoadoutSelection,
    text: LoadoutRecordText,
}

impl LoadoutPathObserver for RecordNames<'_> {
    fn record_path(
        &self,
        selection: &LoadoutSelection,
        parent: &StructValue<'_>,
        value: &Value<'_>,
    ) -> ControlFlow<()> {
        if selection.operation() == LoadoutPathOperation::PortName
            && let Some(text) = LoadoutRecordText::capture(self.forge, parent, value)
        {
            self.pending
                .borrow_mut()
                .entry(ptr::from_ref(value).addr())
                .or_default()
                .push(PendingName {
                    selection: selection.clone(),
                    text,
                });
        }
        ControlFlow::Continue(())
    }

    fn record_name(
        &self,
        _selection: &LoadoutSelection,
        _tree: &RecordTree<'_>,
    ) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }
}
