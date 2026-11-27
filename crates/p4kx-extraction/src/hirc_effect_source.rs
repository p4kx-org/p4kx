// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{
    BankError, BankHircModulator, BankHircObject, BankHircPlugin, BankObservedHeader, BankRtpcRows,
};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::super::node_source::rtpc::{Rtpc, RtpcSchema};
use super::container_layout::ContainerSourceLayout;
use super::origins::ObjectOrigins;

#[path = "hirc_modulator_source.rs"]
pub(super) mod modulator;
#[path = "hirc_plugin_source.rs"]
pub(super) mod plugin;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct MetadataDocument<Kind, Fields, InitialRtpc, Inventory> {
    kind: Kind,
    coverage: Coverage,
    observed: Fields,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<RtpcSchema>")]
    initial_rtpc: Option<InitialRtpc>,
    #[schemars(with = "SourceInventory")]
    source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct Coverage {
    object_kind: SourcedValue<&'static str>,
    layout: SourcedValue<ContainerSourceLayout>,
    version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SourcedValue<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unprojected_range: Option<SourcedValue<StateRange>>,
}

impl Coverage {
    fn checked(
        origins: &ObjectOrigins,
        header: &BankObservedHeader,
        object: &BankHircObject,
        error: Option<&BankError>,
        checked: Range<u64>,
        unprojected: Option<Range<u64>>,
    ) -> Result<Self, SourceEvidenceError> {
        let layout = error.map_or(ContainerSourceLayout::Decoded, |_| {
            ContainerSourceLayout::Malformed
        });
        let mut coverage = Self::header(origins, header, object, layout)?;
        coverage.error = error
            .map(|error| origins.coverage(error.to_string(), header, object))
            .transpose()?;
        coverage.checked_range = Some(origins.range(&checked, "metadataCheckedExtent")?);
        coverage.unprojected_range = unprojected
            .map(|range| origins.range(&range, "metadataUnprojectedExtent"))
            .transpose()?;
        Ok(coverage)
    }

    fn unsupported(
        origins: &ObjectOrigins,
        header: &BankObservedHeader,
        object: &BankHircObject,
    ) -> Result<Self, SourceEvidenceError> {
        let mut coverage =
            Self::header(origins, header, object, ContainerSourceLayout::Unsupported)?;
        let body = object.body_offset()..object.body_end_exclusive();
        coverage.unprojected_range = Some(origins.range(&body, "metadataUnprojectedExtent")?);
        Ok(coverage)
    }

    fn header(
        origins: &ObjectOrigins,
        header: &BankObservedHeader,
        object: &BankHircObject,
        layout: ContainerSourceLayout,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            object_kind: origins.kind(header, object)?,
            layout: origins.coverage(layout, header, object)?,
            version: origins.version(header)?,
            error: None,
            checked_range: None,
            unprojected_range: None,
        })
    }
}

enum InitialRtpc<'view, 'input, R> {
    Plugin(
        &'view RefCell<BankHircPlugin<'input, R>>,
        &'view ObjectOrigins,
    ),
    Modulator(
        &'view RefCell<BankHircModulator<'input, R>>,
        &'view ObjectOrigins,
    ),
}

impl<R: Read + Seek> Serialize for InitialRtpc<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Plugin(reader, origins) => {
                let mut reader = reader.try_borrow_mut().map_err(S::Error::custom)?;
                write_rtpc(origins, reader.rtpc(), serializer)
            }
            Self::Modulator(reader, origins) => {
                let mut reader = reader.try_borrow_mut().map_err(S::Error::custom)?;
                write_rtpc(origins, reader.rtpc(), serializer)
            }
        }
    }
}

fn write_rtpc<R: Read + Seek, S: Serializer>(
    origins: &ObjectOrigins,
    rows: Option<BankRtpcRows<'_, R>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    let rows =
        rows.ok_or_else(|| S::Error::custom("the checked original RTPC section is absent"))?;
    Rtpc::read::<S::Error>(origins, rows)?.serialize(serializer)
}
