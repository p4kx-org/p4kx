// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Guid;

use super::{OriginalDatum, OriginalInstanceLocation};

/// Target facts admitted by the actual same-field canonical decoder operation.
pub(crate) struct OriginalReferenceCapture<'source> {
    pub(crate) guid: OriginalDatum<Guid>,
    pub(crate) record_index: OriginalDatum<u32>,
    pub(crate) record_path: OriginalDatum<&'source str>,
    pub(crate) shape: OriginalTargetShape<'source>,
}

/// Record-header applicability stays distinct from an owned-instance target.
pub(crate) enum OriginalTargetShape<'source> {
    Record {
        source_type: OriginalDatum<&'source str>,
    },
    Instance {
        source_type: OriginalDatum<&'source str>,
        location: OriginalDatum<OriginalInstanceLocation<'source>>,
    },
}
