// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircObject, BankHircPlugin, BankNodeFxKind, BankObservedHeader, BankPluginParameters,
};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::super::super::node_source::routing::NodeFx;
use super::super::super::node_source::state::InlineStateSchema;
use super::{Coverage, InitialRtpc, MetadataDocument, ObjectOrigins, RtpcSchema};

#[path = "hirc_plugin_source_rows.rs"]
mod rows;

use self::rows::{InlineStateGroup, PluginRow, PluginRows};

pub(in crate::hirc_view) type PluginSourceSchema = MetadataDocument<
    PluginTag,
    ObservedPlugin<Vec<PluginRow>, InlineStateSchema>,
    RtpcSchema,
    SourceInventory,
>;

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(in crate::hirc_view) enum PluginTag {
    Plugin,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct ObservedPlugin<Rows, InlineState> {
    #[serde(skip_serializing_if = "Option::is_none")]
    plugin_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameters: Option<PluginParameters>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bank_data_count: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    override_count: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    device_effects: Option<NodeFx>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<InlineStateSchema>")]
    inline_state: Option<InlineState>,
    #[schemars(with = "Vec<PluginRow>")]
    rows: Rows,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PluginParameters {
    declared_size: SourcedValue<u32>,
    range: SourcedValue<StateRange>,
    complete: SourcedValue<bool>,
}

impl PluginParameters {
    fn read(
        origins: &ObjectOrigins,
        parameters: &BankPluginParameters,
    ) -> Result<Self, SourceEvidenceError> {
        let field = &parameters.size;
        Ok(Self {
            declared_size: origins.field(field, "pluginParametersSize")?,
            range: origins.range(&parameters.range, "pluginParametersBytes")?,
            complete: origins.derived(
                parameters.is_complete(),
                &(field.range.start..parameters.range.end),
                "pluginParametersCoverage",
            )?,
        })
    }
}

pub(in crate::hirc_view) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    mut reader: BankHircPlugin<'_, R>,
) -> Result<S::Ok, S::Error> {
    let coverage = Coverage::checked(
        origins,
        header,
        reader.object(),
        reader.format_error(),
        reader.checked_range(),
        reader.unprojected_range(),
    )
    .map_err(S::Error::custom)?;
    let mut device_effects = reader
        .device_slots()
        .map(|slots| NodeFx::observe::<S::Error>(origins, slots.list(), BankNodeFxKind::Effects))
        .transpose()?
        .flatten();
    if let Some(effects) = &mut device_effects {
        effects.read_rows(origins, reader.effect_slots())?;
    }
    let reader = RefCell::new(reader);
    let source = reader.try_borrow().map_err(S::Error::custom)?;
    let observed = observed(origins, &reader, &source, device_effects).map_err(S::Error::custom)?;
    let initial_rtpc = source
        .effect()
        .rtpc
        .is_some()
        .then_some(InitialRtpc::Plugin(&reader, origins));
    drop(source);
    MetadataDocument {
        kind: PluginTag::Plugin,
        coverage,
        observed,
        initial_rtpc,
        source_inventory: origins,
    }
    .serialize(serializer)
}

fn observed<'view, 'input, R>(
    origins: &'view ObjectOrigins,
    reader: &'view RefCell<BankHircPlugin<'input, R>>,
    current: &BankHircPlugin<'input, R>,
    device_effects: Option<NodeFx>,
) -> Result<
    ObservedPlugin<PluginRows<'view, 'input, R>, InlineStateGroup<'view, 'input, R>>,
    SourceEvidenceError,
> {
    let fields = current.effect();
    Ok(ObservedPlugin {
        plugin_id: origins.optional(fields.plugin.plugin_id.as_ref(), "pluginId")?,
        parameters: fields
            .plugin
            .parameters
            .as_ref()
            .map(|parameters| PluginParameters::read(origins, parameters))
            .transpose()?,
        bank_data_count: origins.optional(
            fields.plugin.bank_data_count.as_ref(),
            "pluginBankDataCount",
        )?,
        override_count: origins.optional(fields.override_count.as_ref(), "pluginOverrideCount")?,
        device_effects,
        inline_state: fields
            .states
            .as_ref()
            .map(|_| InlineStateGroup { reader, origins }),
        rows: PluginRows { reader, origins },
    })
}

pub(in crate::hirc_view) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    MetadataDocument {
        kind: PluginTag::Plugin,
        coverage: Coverage::unsupported(origins, header, object).map_err(S::Error::custom)?,
        observed: ObservedPlugin {
            plugin_id: None,
            parameters: None,
            bank_data_count: None,
            override_count: None,
            device_effects: None,
            inline_state: None::<InlineStateSchema>,
            rows: [] as [PluginRow; 0],
        },
        initial_rtpc: None::<RtpcSchema>,
        source_inventory: origins,
    }
    .serialize(serializer)
}
