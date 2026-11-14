// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::sync::Arc;

use crate::{LoadoutFileObservation, LoadoutPath};

#[derive(Debug)]
pub(crate) struct XmlInclude {
    pub(crate) parent: LoadoutFileObservation,
    pub(crate) child: LoadoutFileObservation,
    pub(crate) path: LoadoutPath,
}

#[derive(Debug)]
pub(crate) struct XmlRoute {
    edge: Arc<XmlInclude>,
    tail: Option<Arc<Self>>,
}

impl XmlRoute {
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

#[derive(Debug)]
struct XmlRoot {
    original: LoadoutFileObservation,
    caller: LoadoutPath,
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
        std::iter::successors(self.route.as_deref(), |&route| route.tail.as_deref())
            .map(|route| (&route.edge.parent, &route.edge.child, &route.edge.path))
    }

    pub(crate) fn root(original: LoadoutFileObservation, caller: &LoadoutPath) -> Self {
        Self {
            root: Arc::new(XmlRoot {
                original,
                caller: caller.clone(),
            }),
            route: None,
        }
    }

    pub(crate) fn with_route(&self, route: Option<Arc<XmlRoute>>) -> Self {
        Self {
            root: Arc::clone(&self.root),
            route: route.or_else(|| self.route.clone()),
        }
    }
}
