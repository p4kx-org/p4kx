// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::AtomicUsize;

use p4kx_extraction::Archive;

use super::{Body, Bounds, parse};
use crate::CatalogError;
use crate::catalog_read::original::EntryCapture;
use crate::starmap_index::Index;

pub(crate) fn read_jump_witness_body(
    archive: &Archive,
    index: &Index<'_>,
    entry: usize,
    capture: &EntryCapture<'_>,
) -> Result<Body, CatalogError> {
    let bounds = Bounds {
        cancel: None,
        depth: usize::MAX,
        children: AtomicUsize::new(usize::MAX),
        jump_endpoints: true,
    };
    parse(archive, index, entry, &bounds, capture).map(|(body, _)| body)
}
