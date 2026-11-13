// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankActionFields, BankActionRead, BankHircAction, BankHircActionProperties, BankHircObject,
    BankNodePropertyKind, BankObservedHeader, BankPlayBankDecision, BankPlayBankFields,
    BankPlayProperties,
};
use schemars::JsonSchema;
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::super::node_property::{PropertyList, PropertyWords};
use super::origins::ObjectOrigins;

pub(super) type ActionSourceSchema = ActionDocument<SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "ActionObjectSourceFields")]
pub(super) struct ActionDocument<Inventory> {
    kind: ActionKind,
    version: SourcedValue<u32>,
    layout: SourcedValue<Layout>,
    raw_body_range: SourcedValue<StateRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SourcedValue<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    action_type: Option<SourcedValue<u16>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    target_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bank_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    bank_type: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    play: Option<PlayFields>,
    #[schemars(with = "SourceInventory")]
    source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum ActionKind {
    Action,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Layout {
    Decoded,
    Malformed,
    Absent,
    Unsupported,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PlayFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    target_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fade_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    properties: Option<PropertyList<PropertyWords>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ranged_modifiers: Option<PropertyList<PropertyWords>>,
}

pub(super) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    mut action: BankHircActionProperties<'_, R>,
) -> Result<S::Ok, S::Error> {
    let fields = action.fields();
    let target_flags = origins
        .optional(fields.target_flags.as_ref(), "playTargetFlags")
        .map_err(S::Error::custom)?;
    let fade_flags = origins
        .optional(fields.fade_flags.as_ref(), "playFadeFlags")
        .map_err(S::Error::custom)?;
    let checked_range = fields
        .complete_range()
        .map(|range| origins.range(&range, "playPropertyExtent"))
        .transpose()
        .map_err(S::Error::custom)?;
    let play = PlayFields {
        target_flags,
        fade_flags,
        checked_range,
        properties: properties::<_, S::Error>(
            origins,
            &mut action,
            BankNodePropertyKind::Properties,
        )?,
        ranged_modifiers: properties::<_, S::Error>(
            origins,
            &mut action,
            BankNodePropertyKind::RangedModifiers,
        )?,
    };
    let action = action.action();
    project(
        serializer,
        origins,
        header,
        &action.object,
        Some(&action.status),
        Some(play),
    )
}

pub(super) fn unobserved<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    action: &BankHircAction,
) -> Result<S::Ok, S::Error> {
    project(
        serializer,
        origins,
        header,
        &action.object,
        Some(&action.status),
        None,
    )
}

pub(super) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    project(serializer, origins, header, object, None, None)
}

struct Disposition<'a> {
    layout: Layout,
    error: Option<&'a p4kx_audio::BankError>,
    fields: Option<&'a BankActionFields>,
    bank: Option<&'a BankPlayBankFields>,
}

impl<'a> Disposition<'a> {
    fn read<E: serde::ser::Error>(
        version: u32,
        status: Option<&'a BankActionRead>,
        complete_properties: bool,
    ) -> Result<Self, E> {
        let (fields, error) = match status {
            Some(BankActionRead::Decoded { fields, .. }) => (Some(fields), None),
            Some(BankActionRead::Malformed { fields, error }) => (Some(fields), Some(error)),
            Some(BankActionRead::Unsupported { error }) => (None, Some(error)),
            None => (None, None),
            _ => return Err(E::custom("unexpected Action property disposition")),
        };
        let decision = fields.map(|fields| &fields.play_bank);
        let layout = match decision {
            _ if !BankPlayProperties::supports_version(version) => Layout::Unsupported,
            None | Some(BankPlayBankDecision::VersionGated) => Layout::Unsupported,
            Some(BankPlayBankDecision::NonPlay) => Layout::Absent,
            Some(_) if complete_properties => Layout::Decoded,
            Some(_) if error.is_some() => Layout::Malformed,
            _ => {
                return Err(E::custom(
                    "the observed Play property declaration is absent",
                ));
            }
        };
        let bank = match decision {
            Some(BankPlayBankDecision::Play(bank)) => Some(bank),
            _ => None,
        };
        Ok(Self {
            layout,
            error,
            fields,
            bank,
        })
    }
}

fn project<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
    status: Option<&BankActionRead>,
    play: Option<PlayFields>,
) -> Result<S::Ok, S::Error> {
    let complete_properties = play
        .as_ref()
        .is_some_and(|fields| fields.checked_range.is_some());
    let original =
        Disposition::read::<S::Error>(header.header().version, status, complete_properties)?;
    ActionDocument {
        kind: ActionKind::Action,
        version: origins.version(header).map_err(S::Error::custom)?,
        layout: origins
            .coverage(original.layout, header, object)
            .map_err(S::Error::custom)?,
        raw_body_range: origins
            .range(
                &(object.body_offset()..object.body_end_exclusive()),
                "actionRawBodyExtent",
            )
            .map_err(S::Error::custom)?,
        error: original
            .error
            .map(|error| origins.coverage(error.to_string(), header, object))
            .transpose()
            .map_err(S::Error::custom)?,
        action_type: origins
            .optional(
                original
                    .fields
                    .and_then(|fields| fields.action_type.as_ref()),
                "actionType",
            )
            .map_err(S::Error::custom)?,
        target_id: origins
            .optional(
                original.fields.and_then(|fields| fields.target_id.as_ref()),
                "actionTargetId",
            )
            .map_err(S::Error::custom)?,
        bank_id: origins
            .optional(
                original.bank.and_then(|bank| bank.bank_id.as_ref()),
                "actionPlayBankId",
            )
            .map_err(S::Error::custom)?,
        bank_type: origins
            .optional(
                original.bank.and_then(|bank| bank.bank_type.as_ref()),
                "actionPlayBankType",
            )
            .map_err(S::Error::custom)?,
        play,
        source_inventory: origins,
    }
    .serialize(serializer)
}

fn properties<R: Read + Seek, E: serde::ser::Error>(
    origins: &ObjectOrigins,
    action: &mut BankHircActionProperties<'_, R>,
    kind: BankNodePropertyKind,
) -> Result<Option<PropertyList<PropertyWords>>, E> {
    PropertyList::read(origins, action.properties(kind), |row| {
        PropertyWords::read(origins, &row)
    })
}
