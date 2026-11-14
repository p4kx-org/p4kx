// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::io::Cursor;
use std::ops::Range;

use p4kx_extraction::bank::{
    BankHircActorMixer, BankHircContainerNode, BankHircField, BankHircSoundNode, BankNodeAuxFields,
    BankNodeFxKind, BankNodeParentFields, BankNodePlaybackFields, BankNodePropertyExtents,
    BankNodePropertyKind, BankRtpcRead, BankSoundStateRead, StateObject,
};
use p4kx_media::MediaRecord;
use serde_json::{Value, json};

use super::{TestResult, offset, require};

#[path = "audio_node_compare.rs"]
mod compare;
#[path = "audio_node_modulation.rs"]
mod modulation;

pub(super) use self::music::music;

type Owner = [u64; 2];
type Input<'a> = Cursor<&'a [u8]>;
const PROPERTIES: &str = "audio-bank-hirc-node-properties";
const AUX: &str = "audio-bank-hirc-node-aux";
const POSITIONING: &str = "audio-bank-hirc-node-positioning";
const ADVANCED: &str = "audio-bank-hirc-node-advanced-settings";

#[derive(Clone, Copy)]
#[repr(u8)]
pub(super) enum Coverage {
    Sound,
    ActorMixer,
    Container,
    Priority,
    Properties,
    Ranged,
    Fx,
    Paths,
    Routing,
    Advanced,
    InlineState,
    Rtpc,
    StandaloneState,
    TypedStandaloneState,
    Music,
}

#[derive(Clone, Copy)]
pub(super) struct Common<'a> {
    pub(super) parent: &'a BankNodeParentFields,
    pub(super) properties: &'a BankNodePropertyExtents,
    pub(super) aux: &'a BankNodeAuxFields,
    pub(super) playback: &'a BankNodePlaybackFields,
}

pub(super) struct Visitor<'a> {
    coverage: u16,
    output: Option<Output<'a>>,
}

struct Output<'a> {
    input: &'a [u8],
    rows: BTreeMap<(&'a str, &'a [u64]), &'a MediaRecord>,
}

impl<'a> Visitor<'a> {
    pub(super) const fn scan() -> Self {
        Self {
            coverage: 0,
            output: None,
        }
    }

    pub(super) fn verify(input: &'a [u8], rows: &'a [MediaRecord]) -> TestResult<Self> {
        let mut indexed = BTreeMap::new();
        for row in rows {
            require(
                indexed
                    .insert(
                        (
                            row.subresource.namespace.as_str(),
                            row.subresource.indices.as_slice(),
                        ),
                        row,
                    )
                    .is_none(),
                "one row per physical audio subresource",
            )?;
        }
        Ok(Self {
            coverage: 0,
            output: Some(Output {
                input,
                rows: indexed,
            }),
        })
    }

    pub(super) const fn mark(&mut self, kind: Coverage) {
        self.coverage |= 1 << kind as u8;
    }

    pub(super) const fn has(&self, kind: Coverage) -> bool {
        self.coverage & (1 << kind as u8) != 0
    }

    pub(super) const fn include(&mut self, other: &Self) {
        self.coverage |= other.coverage;
    }

    pub(super) const fn score(&self) -> u32 {
        self.coverage.count_ones()
    }

    pub(super) fn has_complete_node_groups(&self) -> bool {
        [
            Coverage::Sound,
            Coverage::ActorMixer,
            Coverage::Container,
            Coverage::Priority,
            Coverage::Properties,
            Coverage::Ranged,
            Coverage::Fx,
            Coverage::Paths,
            Coverage::Routing,
            Coverage::Advanced,
            Coverage::InlineState,
            Coverage::Rtpc,
        ]
        .into_iter()
        .all(|kind| self.has(kind))
    }

    pub(super) fn common(&mut self, owner: Owner, source: Common<'_>) -> TestResult {
        self.mark_lists(source);
        let positioning = &source.playback.positioning;
        if positioning
            .vertex_count
            .as_ref()
            .is_some_and(|count| count.value != 0)
        {
            self.mark(Coverage::Paths);
        }
        if source
            .aux
            .user_buses_range
            .as_ref()
            .is_some_and(|range| !range.is_empty())
            || source
                .parent
                .pre_parent_word
                .as_ref()
                .is_some_and(|id| id.value != 0)
            || source
                .aux
                .reflections_bus_id
                .as_ref()
                .is_some_and(|id| id.value != 0)
        {
            self.mark(Coverage::Routing);
        }
        if source
            .playback
            .advanced
            .flags
            .as_ref()
            .is_some_and(|flags| flags.value != 0)
        {
            self.mark(Coverage::Advanced);
        }
        if let Some(output) = &self.output {
            compare::common(output, owner, source)?;
        }
        Ok(())
    }

    fn mark_lists(&mut self, source: Common<'_>) {
        if source
            .parent
            .priority_flags
            .as_ref()
            .is_some_and(|flags| flags.value != 0)
        {
            self.mark(Coverage::Priority);
        }
        for (kind, coverage) in [
            (BankNodePropertyKind::Properties, Coverage::Properties),
            (BankNodePropertyKind::RangedModifiers, Coverage::Ranged),
        ] {
            if source
                .properties
                .get(kind)
                .is_some_and(|extent| extent.declared_count.value != 0)
            {
                self.mark(coverage);
            }
        }
        for kind in [BankNodeFxKind::Effects, BankNodeFxKind::Metadata] {
            if source
                .parent
                .fx(kind)
                .declared_count
                .is_some_and(|count| count.value != 0)
            {
                self.mark(Coverage::Fx);
            }
        }
    }
}

impl Output<'_> {
    fn row(&self, namespace: &str, indices: &[u64]) -> TestResult<&MediaRecord> {
        self.rows
            .get(&(namespace, indices))
            .copied()
            .ok_or_else(|| "missing typed node field at original physical position".into())
    }

    fn field<T: serde::Serialize + Copy>(
        &self,
        row: &MediaRecord,
        path: &str,
        original: Option<&BankHircField<T>>,
    ) -> TestResult {
        let emitted = row.data.pointer(path);
        let Some(original) = original else {
            return require(emitted.is_none(), "unread node field remains absent");
        };
        let emitted = emitted.ok_or("original checked node field")?;
        require(
            emitted.get("value") == Some(&serde_json::to_value(original.value)?),
            "typed node value matches the original word",
        )?;
        self.extent(
            emitted.get("range").ok_or("original field range")?,
            &original.range,
        )
    }

    fn extent(&self, emitted: &Value, original: &Range<u64>) -> TestResult {
        let start = usize::try_from(original.start)?;
        let end = usize::try_from(original.end)?;
        require(
            emitted.pointer("/basis/type") == Some(&json!("entry"))
                && offset(emitted, "/start")? == start
                && offset(emitted, "/end_exclusive")? == end
                && self.input.get(start..end).is_some(),
            "node field references its exact original retained bytes",
        )
    }

    fn group_extent(&self, row: &MediaRecord, key: &str, original: &Range<u64>) -> TestResult {
        self.extent(
            row.data.pointer(key).ok_or("typed node group extent")?,
            original,
        )
    }
}

macro_rules! verify_node_rows {
    ($output:expr, $owner:expr, $node:expr) => {{
        let output = $output;
        let owner = $owner;
        let node = $node;
        for (list, kind) in compare::PROPERTY_LISTS.into_iter().enumerate() {
            compare::properties(output, owner, u64::try_from(list)?, node.properties(kind))?;
        }
        for (list, kind) in compare::EFFECT_LISTS.into_iter().enumerate() {
            compare::effects(output, owner, u64::try_from(list)?, node.fx_slots(kind))?;
        }
        for (table, kind) in compare::POSITIONING_TABLES.into_iter().enumerate() {
            compare::paths(
                output,
                owner,
                u64::try_from(table)?,
                node.positioning_rows(kind),
            )?;
        }
        compare::aux(output, owner, node.user_aux_buses()?)
    }};
}

#[path = "audio_node_music.rs"]
mod music;

pub(super) fn actor(
    visitor: &mut Visitor<'_>,
    owner: Owner,
    node: &mut BankHircActorMixer<'_, Input<'_>>,
) -> TestResult {
    if let Some(output) = &visitor.output {
        let row = output.row("audio-bank-hirc-actor-mixer", &owner)?;
        output.field(
            row,
            "/payload/child_count",
            node.fields().child_count.as_ref(),
        )?;
        if let Some(range) = &node.fields().node_range {
            output.group_extent(row, "/payload/checked_node_range", range)?;
        }
        if let Some(children) = node.children() {
            for child in children {
                let child = child?;
                let [section, object] = owner;
                let row = output.row(
                    "audio-bank-hirc-actor-mixer-child",
                    &[section, object, u64::from(child.ordinal)],
                )?;
                output.field(row, "/payload/child_id", Some(&child.child_id))?;
            }
        }
    }
    modulation::state(visitor, owner, node.states())?;
    modulation::rtpc(visitor, owner, "actor-mixer", node.rtpc())?;
    if let Some(output) = &visitor.output {
        verify_node_rows!(output, owner, node)?;
    }
    Ok(())
}

pub(super) fn container(
    visitor: &mut Visitor<'_>,
    owner: Owner,
    node: &mut BankHircContainerNode<'_, Input<'_>>,
) -> TestResult {
    modulation::state(visitor, owner, node.states())?;
    modulation::rtpc(visitor, owner, "container", node.rtpc())?;
    if let Some(output) = &visitor.output {
        verify_node_rows!(output, owner, node)?;
    }
    Ok(())
}

pub(super) fn sound(
    visitor: &mut Visitor<'_>,
    owner: Owner,
    node: &mut BankHircSoundNode<'_, Input<'_>>,
) -> TestResult {
    if let BankSoundStateRead::Readable(rows) = node.states() {
        modulation::state(visitor, owner, Some(rows))?;
    }
    match node.rtpc() {
        BankRtpcRead::Readable(rows) | BankRtpcRead::Malformed { rows, .. } => {
            modulation::rtpc(visitor, owner, "sound", Some(rows))?;
        }
        _ => {}
    }
    if let Some(output) = &visitor.output {
        verify_node_rows!(output, owner, node)?;
    }
    Ok(())
}

pub(super) fn standalone(
    visitor: &Visitor<'_>,
    owner: Owner,
    version: u32,
    source: Option<&StateObject>,
) -> TestResult {
    let Some(output) = &visitor.output else {
        return Ok(());
    };
    let row = output.row("audio-bank-hirc-state", &owner)?;
    let layout = if source.is_some() {
        "decoded"
    } else {
        "unsupported"
    };
    require(
        row.data.pointer("/payload/layout") == Some(&json!(layout))
            && row.data.pointer("/payload/version") == Some(&json!(version)),
        "standalone State version and checked layout",
    )?;
    if let Some(state) = source {
        output.field(
            row,
            "/payload/property_count",
            Some(state.properties.count()),
        )?;
        output.group_extent(
            row,
            "/payload/consumed_range",
            &state.properties.consumed_range(),
        )?;
        output.group_extent(row, "/payload/trailing_range", &state.trailing_range)?;
        modulation::state_values(
            output,
            "audio-bank-hirc-state-property",
            &owner,
            &state.properties,
        )?;
    }
    Ok(())
}
