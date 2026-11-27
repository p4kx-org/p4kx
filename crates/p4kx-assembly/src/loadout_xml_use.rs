// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, SourceLocation};

use crate::{LoadoutClassSelection, LoadoutFileObservation, LoadoutPath};

#[path = "paint_xml_loadout_attributes.rs"]
mod capture;

pub(crate) use capture::XmlLoadoutAttributes;

#[derive(Debug)]
pub(crate) struct XmlInclude {
    pub(crate) parent: LoadoutFileObservation,
    pub(crate) child: LoadoutFileObservation,
    pub(crate) path: LoadoutPath,
}

pub(crate) struct XmlRoute {
    edge: Arc<XmlInclude>,
    tail: Option<Arc<Self>>,
}

impl XmlRoute {
    pub(crate) fn includes(
        route: Option<&Self>,
    ) -> impl Iterator<
        Item = (
            &LoadoutFileObservation,
            &LoadoutFileObservation,
            &LoadoutPath,
        ),
    > {
        std::iter::successors(route, |&route| route.tail.as_deref())
            .map(|route| (&route.edge.parent, &route.edge.child, &route.edge.path))
    }

    pub(crate) fn prepend(
        tail: Option<Arc<Self>>,
        edge: &Arc<XmlInclude>,
        routes: &mut HashMap<Option<usize>, Arc<Self>>,
    ) -> Arc<Self> {
        let key = tail.as_ref().map(|tail| Arc::as_ptr(tail).addr());
        Arc::clone(routes.entry(key).or_insert_with(|| {
            Arc::new(Self {
                edge: Arc::clone(edge),
                tail,
            })
        }))
    }
}

impl Drop for XmlRoute {
    fn drop(&mut self) {
        let mut tail = self.tail.take();
        while let Some(mut next) = tail.take().and_then(Arc::into_inner) {
            tail = next.tail.take();
        }
    }
}

impl fmt::Debug for XmlRoute {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_list()
            .entries(
                std::iter::successors(Some(self), |&route| route.tail.as_deref())
                    .map(|route| &route.edge),
            )
            .finish()
    }
}

#[cfg(test)]
#[path = "loadout_xml_route_tests.rs"]
mod route_tests;

#[derive(Debug)]
struct XmlRoot {
    original: LoadoutFileObservation,
    caller: LoadoutPath,
    paint_attributes: Option<Arc<XmlLoadoutAttributes>>,
}

/// The actual caller and Include route that supplied an XML class selection.
#[derive(Debug, Clone)]
pub struct LoadoutXmlClassUse {
    root: Arc<XmlRoot>,
    route: Option<Arc<XmlRoute>>,
}

impl LoadoutXmlClassUse {
    /// Borrows the actual root XML operation retained by this call.
    #[must_use]
    pub fn original_root(&self) -> &LoadoutFileObservation {
        &self.root.original
    }

    /// Borrows the actual current caller's XML request.
    #[must_use]
    pub fn caller(&self) -> &LoadoutPath {
        &self.root.caller
    }

    /// Visits the genuine selected Include edges from root to the selected Entry.
    pub fn includes(
        &self,
    ) -> impl Iterator<
        Item = (
            &LoadoutFileObservation,
            &LoadoutFileObservation,
            &LoadoutPath,
        ),
    > {
        XmlRoute::includes(self.route.as_deref())
    }

    pub(crate) fn root(
        original: LoadoutFileObservation,
        caller: &LoadoutPath,
        paint_attributes: Option<Arc<XmlLoadoutAttributes>>,
    ) -> Self {
        Self {
            root: Arc::new(XmlRoot {
                original,
                caller: caller.clone(),
                paint_attributes,
            }),
            route: None,
        }
    }

    pub(crate) fn with_caller(&self, caller: &LoadoutPath) -> Self {
        Self::root(
            self.root.original.clone(),
            caller,
            self.root.paint_attributes.clone(),
        )
    }

    pub(crate) fn paint_attributes(
        &self,
        selection: &LoadoutClassSelection,
    ) -> Option<Vec<(EntryOrigin, SourceLocation)>> {
        self.root
            .paint_attributes
            .as_ref()?
            .selected(self, selection)
    }

    pub(crate) fn with_route(&self, route: Option<Arc<XmlRoute>>) -> Self {
        Self {
            root: Arc::clone(&self.root),
            route: route.or_else(|| self.route.clone()),
        }
    }
}
