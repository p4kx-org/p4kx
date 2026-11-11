// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::bank::{
    BankHircObject, BankHircState, BankStateRead, StateObject, StateProperties,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_state_payload::{StandaloneStateRecord, StatePayload, StatePropertyRecord};
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) const STATE_NAMESPACE: &str = "audio-bank-hirc-state";
pub(crate) const PROPERTY_NAMESPACE: &str = "audio-bank-hirc-state-property";
pub(crate) const INLINE_PROPERTY_NAMESPACE: &str = "audio-bank-hirc-inline-state-property";

pub(crate) fn project(
    input: &MediaOriginInput,
    version: VersionSite,
    state: BankHircState,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let projection = StateProjection::new(input, &state.object, version);
    let mut prepared = projection.record(writer, None)?;
    let version = version.version;
    let (record, issue, properties) = match state.status {
        BankStateRead::Decoded(state) => {
            let record = decoded_record(&projection, &mut prepared, &state)?;
            (record, None, Some(state.properties))
        }
        BankStateRead::Unsupported { .. } => {
            projection.sites.summary(&mut prepared, None, false)?;
            (StandaloneStateRecord::Unsupported { version }, None, None)
        }
        BankStateRead::Malformed(cause) => {
            projection
                .sites
                .summary(&mut prepared, Some(&projection.body_range), true)?;
            let issue = MediaIssue::new(
                "audio.bank.invalid-state",
                "HIRC",
                Severity::Error,
                cause.to_string(),
            );
            (
                StandaloneStateRecord::Malformed { version },
                Some(issue),
                None,
            )
        }
        _ => {
            return Err(MediaError::Contract(
                "unexpected standalone State disposition".into(),
            ));
        }
    };
    let status = if issue.is_some() {
        Status::Partial
    } else {
        Status::Complete
    };
    prepared.finish().write(
        writer,
        &Subresource::new(STATE_NAMESPACE, projection.indices.to_vec()),
        status,
        &StatePayload::Standalone(record),
        issue.as_slice(),
    )?;
    if let Some(properties) = properties {
        projection.properties(&properties, PropertyScope::Standalone, writer)?;
    }
    Ok(())
}

fn decoded_record(
    projection: &StateProjection<'_>,
    prepared: &mut BankOrigins<'_, '_, '_>,
    state: &StateObject,
) -> Result<StandaloneStateRecord, MediaError> {
    let extent = state.properties.consumed_range();
    projection.sites.summary(prepared, Some(&extent), false)?;
    projection.property_header(prepared, &state.properties, &extent)?;
    projection
        .sites
        .range(prepared, "trailing_range", &state.trailing_range)?;
    Ok(StandaloneStateRecord::Decoded {
        version: projection.version,
        property_count: state.properties.count().into(),
        consumed_range: (&extent).into(),
        trailing_range: (&state.trailing_range).into(),
    })
}

#[derive(Clone, Copy)]
pub(crate) enum PropertyScope {
    Standalone,
    Inline { group: u32, state: u32 },
}

pub(crate) struct StateProjection<'source> {
    input: &'source MediaOriginInput,
    pub(crate) sites: NodeSites,
    pub(crate) indices: [u64; 2],
    pub(crate) version: u32,
    pub(crate) body_range: Range<u64>,
}

impl<'source> StateProjection<'source> {
    pub(crate) const fn new(
        input: &'source MediaOriginInput,
        object: &BankHircObject,
        version: VersionSite,
    ) -> Self {
        Self {
            input,
            sites: NodeSites::new(object, version, NodeKind::State),
            indices: [object.section_ordinal(), object.ordinal()],
            version: version.version,
            body_range: object.body_offset()..object.body_end_exclusive(),
        }
    }

    pub(crate) fn record<'writer, 'run>(
        &self,
        writer: &'writer MediaWriter<'run>,
        first_read: Option<&Range<u64>>,
    ) -> Result<BankOrigins<'source, 'writer, 'run>, MediaError> {
        self.sites.record(self.input, writer, first_read)
    }

    pub(crate) fn property_header(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        properties: &StateProperties,
        extent: &Range<u64>,
    ) -> Result<(), MediaError> {
        self.sites.field(
            prepared,
            "property_count",
            &properties.count().range,
            "statePropertyCount",
        )?;
        self.sites.range(prepared, "consumed_range", extent)
    }

    pub(crate) fn properties(
        &self,
        properties: &StateProperties,
        scope: PropertyScope,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let [section, object] = self.indices;
        for (ordinal, property) in (0_u64..).zip(properties.entries()) {
            let mut prepared = self.record(writer, Some(&property.id.range))?;
            self.sites.field(
                &mut prepared,
                "property_id",
                &property.id.range,
                "statePropertyId",
            )?;
            self.sites.field(
                &mut prepared,
                "value_bits",
                &property.value_bits.range,
                "stateValueBits",
            )?;
            self.sites.converted(
                &mut prepared,
                "value",
                &property.value_bits.range,
                "stateValueBits",
            )?;
            let value = property.value();
            let record = StatePropertyRecord {
                property_id: (&property.id).into(),
                value_bits: (&property.value_bits).into(),
                value: value.is_finite().then_some(value),
            };
            let (subresource, payload) = match scope {
                PropertyScope::Standalone => (
                    Subresource::new(PROPERTY_NAMESPACE, vec![section, object, ordinal]),
                    StatePayload::Property(record),
                ),
                PropertyScope::Inline { group, state } => (
                    Subresource::new(
                        INLINE_PROPERTY_NAMESPACE,
                        vec![section, object, u64::from(group), u64::from(state), ordinal],
                    ),
                    StatePayload::InlineProperty(record),
                ),
            };
            prepared
                .finish()
                .write(writer, &subresource, Status::Complete, &payload, &[])?;
        }
        Ok(())
    }
}
