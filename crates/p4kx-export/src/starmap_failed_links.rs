// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::CancelToken;

use super::{Body, Level};
use crate::catalog_read::original::EntryCapture;
use crate::starmap_index::Index;
use crate::{CatalogError, ExportAccountingError};

pub(super) fn observe(entry: usize, body: &Body, pending: &mut HashSet<usize>) {
    if matches!(
        body,
        Body::Parsed {
            pending_external: true,
            ..
        }
    ) {
        pending.insert(entry);
    }
}

pub(super) fn finish(
    bodies: &mut HashMap<usize, Body>,
    pending: &mut HashSet<usize>,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    checkpoint(cancel)?;
    let mut parents: Vec<_> = pending.iter().copied().collect();
    parents.sort_unstable();
    for parent in parents {
        checkpoint(cancel)?;
        let mut body = bodies
            .remove(&parent)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let waiting = match &mut body {
            Body::Parsed {
                root,
                pending_external,
                ..
            } => {
                let waiting = retain_failed(root, parent, bodies, index, capture, cancel)?;
                *pending_external = waiting;
                waiting
            }
            Body::Unparsed | Body::Unreadable(_) => false,
        };
        bodies.insert(parent, body);
        if !waiting {
            pending.remove(&parent);
        }
    }
    Ok(())
}

fn retain_failed(
    root: &mut Level,
    parent: usize,
    bodies: &HashMap<usize, Body>,
    index: &Index<'_>,
    capture: &EntryCapture<'_>,
    cancel: Option<&CancelToken>,
) -> Result<bool, CatalogError> {
    let mut waiting = false;
    let mut levels = vec![root];
    while let Some(level) = levels.pop() {
        for child in &mut level.children {
            checkpoint(cancel)?;
            if child.deferred_external.is_some() {
                let selector = child
                    .external
                    .as_ref()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                let [target] = index.candidates(selector.as_str()) else {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                };
                let failed = if *target == parent {
                    Some(false)
                } else {
                    bodies
                        .get(target)
                        .map(|body| !matches!(body, Body::Parsed { .. }))
                };
                match failed {
                    None => waiting = true,
                    Some(failed) => {
                        let original = child
                            .deferred_external
                            .take()
                            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                        if failed {
                            child.unresolved_external = Some(original.retain(selector, capture)?);
                        }
                    }
                }
            }
            if !child.below.children.is_empty() {
                levels.push(&mut child.below);
            }
        }
    }
    Ok(waiting)
}

fn checkpoint(cancel: Option<&CancelToken>) -> Result<(), CatalogError> {
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    Ok(())
}

pub(super) fn completed(
    pending: &HashSet<usize>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    checkpoint(cancel)?;
    if !pending.is_empty() {
        return Err(ExportAccountingError::UnobservedFailure.into());
    }
    Ok(())
}
