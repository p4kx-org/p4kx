// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::hirc_node_aux::BankNodeAuxFields;
use crate::hirc_node_playback::BankNodePlaybackFields;
use crate::hirc_node_property::BankNodePropertyExtents;
use crate::hirc_rtpc::BankRtpcExtent;

/// Checked shared field groups following the parent declaration in a version-154 node.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankNodeParameters {
    /// Original planar property and ranged modifier lists.
    pub properties: BankNodePropertyExtents,
    /// Original auxiliary routing declarations.
    pub aux: BankNodeAuxFields,
    /// Original positioning and advanced settings.
    pub playback: BankNodePlaybackFields,
    /// Complete original inline State chunk when readable.
    pub state_chunk: Option<Range<u64>>,
    /// Original RTPC count and checked curve prefix when readable.
    pub rtpc_chunk: Option<BankRtpcExtent>,
}
