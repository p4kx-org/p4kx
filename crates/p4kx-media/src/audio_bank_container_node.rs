// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankContainerRead, BankHircContainerNode, BankHircObject};

use crate::audio_bank_inline_state::{self, InlineStateRead};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declarations::{self, Owner};
use crate::audio_bank_node_properties;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_random_sequence;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::audio_bank_state::StateProjection;
use crate::{MediaError, MediaWriter};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    container: &mut BankHircContainerNode<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_bank_node_properties::container(projection, version, container, writer)?;
    audio_bank_node_declarations::emit(projection, version, Owner::Container(container), writer)?;
    if matches!(container.status(), BankContainerRead::Unsupported(_)) {
        return Ok(());
    }

    let original = *container.object();
    let state = StateProjection::new(projection.input, &original, version);
    let read = container
        .states()
        .map_or(InlineStateRead::Unavailable, InlineStateRead::Readable);
    audio_bank_inline_state::project(projection, &state, read, writer)?;
    let error = match container.status() {
        BankContainerRead::Malformed(error) => Some(error.clone()),
        _ => None,
    };
    let read = audio_bank_rtpc::node_rows(container.rtpc(), error)?;
    audio_bank_rtpc::project(
        projection,
        version,
        &original,
        RtpcOwner::Container,
        read,
        writer,
    )
}

pub(crate) fn unsupported(
    projection: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    audio_bank_node_properties::unsupported(projection, version, original, writer)?;
    audio_bank_node_declarations::unsupported(projection, version, original, writer)?;
    audio_bank_random_sequence::unsupported(projection, version, original, writer)
}
