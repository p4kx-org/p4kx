// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, LoadoutFiles};
use p4kx_extraction::{Archive, Record, RecordTree};

use super::{Case, Prepared, geometry};

/// Prepares a geometry fixture without retaining audio-only state.
pub(super) fn prepare<'f>(
    archive: &Archive,
    files: &'f dyn LoadoutFiles,
    entities: Arc<Entities<'f>>,
    record: &Record<'f>,
    canonical: &RecordTree<'f>,
    case: &Case,
) -> super::Result<Option<Prepared>> {
    geometry::prepare_with_audio(archive, files, entities, record, canonical, case, 0)
}
