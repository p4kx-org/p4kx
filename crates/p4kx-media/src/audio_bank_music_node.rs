// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::BankError;
use p4kx_extraction::bank::{BankHircObject, BankSharedNode};

use crate::audio_bank_inline_state::{self, InlineStateRead};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declarations::{self, Owner};
use crate::audio_bank_node_properties;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::audio_bank_state::StateProjection;
use crate::{MediaError, MediaWriter};

pub(super) fn project<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    object: BankHircObject,
    mut node: BankSharedNode<'_, R>,
    error: Option<&BankError>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_bank_node_properties::shared(bank, version, &object, &mut node, error, writer)?;
    audio_bank_node_declarations::emit(
        bank,
        version,
        Owner::Shared {
            object,
            node: &mut node,
            error,
        },
        writer,
    )?;
    let state = StateProjection::new(bank.input, &object, version);
    let rows = node
        .states()
        .map_or(InlineStateRead::Unavailable, InlineStateRead::Readable);
    audio_bank_inline_state::project(bank, &state, rows, writer)?;
    let rows = audio_bank_rtpc::node_rows(node.rtpc(), error.cloned())?;
    audio_bank_rtpc::project(bank, version, &object, RtpcOwner::MusicNode, rows, writer)
}

pub(super) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_bank_node_properties::unsupported(bank, version, object, writer)?;
    audio_bank_node_declarations::unsupported(bank, version, object, writer)
}
