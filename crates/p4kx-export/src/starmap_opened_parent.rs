// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{SourceUse, SourceUseRole};

use super::WalkPath;
use crate::CatalogError;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::starmap_containers::proof::{PlacementStep, container_route_uses};

/// The retained evidence for the immediately containing authored starmap Record.
#[derive(Debug, Clone)]
pub(crate) enum OpenedParent {
    Root,
    Reached(Box<ReachedParent>),
    /// The directory seed is a fallback whose ancestry needs an exhaustive route survey.
    DirectoryFallback,
    Unobserved,
}

#[derive(Debug, Clone)]
pub(crate) struct ReachedParent {
    route: Arc<PlacementStep>,
    via: AuthoredEntryPath,
}

impl OpenedParent {
    pub(super) fn from_path(path: &WalkPath) -> Self {
        match (&path.route, &path.via) {
            (Some(route), Some(via)) => Self::Reached(Box::new(ReachedParent {
                route: Arc::clone(route),
                via: via.clone(),
            })),
            _ if path.external == 0 => Self::Root,
            _ => Self::Unobserved,
        }
    }

    pub(crate) fn inputs(&self) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        match self {
            Self::Root => Ok((Vec::new(), true)),
            Self::DirectoryFallback | Self::Unobserved => Ok((Vec::new(), false)),
            Self::Reached(parent) => parent.inputs(),
        }
    }
}

impl ReachedParent {
    fn inputs(&self) -> Result<(Vec<SourceUse>, bool), CatalogError> {
        let (mut uses, complete) = container_route_uses(&[], Some(&self.route), Some(&self.via))?;
        for step in self.route.steps() {
            uses.push(step.position(SourceUseRole::Selector)?);
            uses.push(step.rotation(SourceUseRole::Selector)?);
        }
        self.route.record_ancestor_uses(&mut uses)?;
        Ok((
            uses,
            complete && self.route.steps().all(PlacementStep::original_complete),
        ))
    }
}
