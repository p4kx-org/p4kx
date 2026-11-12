// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The view model of a soundbank's `HIRC` objects: what is known of one object, built once and
//! rendered by every front-end as text or JSON, whose field names it fixes.

use p4kx_audio::{HircKind, HircLayout, HircObject, MusicGroupKind, MusicPlayMode};
use schemars::JsonSchema;
use serde::{Serialize, Serializer};

#[path = "hirc_view_build.rs"]
mod build;
#[path = "hirc_source_fields.rs"]
mod source_fields;
#[path = "hirc_view_state.rs"]
mod state;

use crate::bank::Bank;

use self::build::detail;
pub use self::source_fields::ObjectSourceFields;
use self::source_fields::ObjectSourceSchema;
pub use self::state::{StateDetail, StatePropertyView, StateRange};

/// One `HIRC` object.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ObjectView<'a> {
    /// The position in the `HIRC` section.
    pub index: usize,
    /// The object ID.
    pub id: u32,
    /// The type, as [`HircKind::slug`] names it.
    #[serde(rename = "type")]
    pub kind: &'static str,
    /// The type byte, for a type this reader does not know.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_byte: Option<u8>,
    /// How far the reader decodes the object: `decoded`, `undecoded`, `unsupported` or
    /// `malformed`; `status` in JSON.
    #[serde(rename = "status", serialize_with = "serialize_layout")]
    #[schemars(with = "Status")]
    pub layout: HircLayout,
    /// Why the object is unsupported or malformed.
    pub error: Option<String>,
    /// The parent node, `None` at the top or without a parent field.
    pub parent: Option<u32>,
    /// The IDs the object lists, in bank order.
    pub children: Vec<u32>,
    /// The fields that depend on the type.
    pub detail: Option<Detail<'a>>,
    /// Original source fields available for the selected physical object.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<ObjectSourceSchema>")]
    pub source_fields: Option<ObjectSourceFields<'a>>,
}

/// The fields of an object that depend on its type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum Detail<'a> {
    /// A standalone State.
    State(StateDetail),
    /// A Sound.
    Sound(SoundDetail),
    /// An Action.
    Action(ActionDetail),
    /// A random, sequence, switch or blend container.
    Container(ContainerDetail),
    /// An `ActorMixer`.
    ActorMixer(ActorMixerDetail),
    /// A music segment.
    Segment(SegmentDetail<'a>),
    /// A music track.
    Track(TrackDetail<'a>),
    /// A music switch container.
    Switch(SwitchDetail<'a>),
    /// A music playlist container.
    Playlist(PlaylistDetail),
}

/// A Sound's media source.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SoundDetail {
    /// The Wwise plug-in ID of the codec.
    pub plugin_id: u32,
    /// Where the media is stored: `embedded`, `prefetch-streamed`, `streamed` or `other`.
    pub storage: &'static str,
    /// The WEM ID.
    pub source_id: u32,
    /// The size of the media in memory as the bank states it.
    pub in_memory_size: u32,
    /// Whether the WEM lies in the folder of a language.
    pub language_specific: bool,
    /// Whether a plug-in generates the sound, so that no WEM holds it.
    pub source_plugin: bool,
}

/// An Action's type and target.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ActionDetail {
    /// The action type code.
    pub action_type: u16,
    /// The ID of the object the action acts on.
    pub target: u32,
    /// The soundbank a Play action names, 0 for none; `None` for any other action.
    pub soundbank_id: Option<u32>,
}

/// A container's kind.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ContainerDetail {
    /// `random`, `sequence`, `switch`, `blend` or `other`.
    pub container_kind: &'static str,
}

/// Original spans of an `ActorMixer`'s shared node base and ordered children.
#[derive(Debug, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct ActorMixerDetail {
    /// Traversed shared node base, including checked opaque data.
    pub node_range: StateRange,
    /// Original child count word.
    pub child_count_range: StateRange,
    /// Original ordered child ID words.
    pub children_range: StateRange,
}

/// A music segment.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SegmentDetail<'a> {
    /// The duration in milliseconds.
    pub duration_ms: f64,
    /// The tempo in beats per minute.
    pub tempo: f32,
    /// The beats per bar.
    pub beats_per_bar: u8,
    /// The note value of a beat.
    pub beat_value: u8,
    /// The named positions in the segment.
    pub markers: Vec<MarkerView<'a>>,
    /// The triggers of the segment and the segments they play.
    pub stingers: Vec<StingerView>,
}

/// A named position in a segment.
#[derive(Debug, Serialize, JsonSchema)]
pub struct MarkerView<'a> {
    /// The marker ID.
    pub id: u32,
    /// The position in milliseconds.
    pub position_ms: f64,
    /// The marker name.
    pub name: &'a str,
}

/// A segment a trigger plays.
#[derive(Debug, Serialize, JsonSchema)]
pub struct StingerView {
    /// The trigger ID.
    pub trigger_id: u32,
    /// The segment the trigger plays.
    pub segment_id: u32,
}

/// A music track.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TrackDetail<'a> {
    /// `normal`, `random`, `sequence`, `switch` or `other`.
    pub track_kind: &'static str,
    /// The clips of the track.
    pub clips: Vec<ClipView>,
    /// The group a switch track follows.
    pub switch: Option<TrackSwitchView<'a>>,
}

/// A clip of a track; times in milliseconds.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ClipView {
    /// The WEM ID of the clip's source.
    pub source_id: u32,
    /// The event the clip posts, 0 for none.
    pub event_id: u32,
    /// Where the clip starts in the track.
    pub play_at_ms: f64,
    /// How much is cut from the start of the source.
    pub begin_trim_ms: f64,
    /// How much is cut from the end of the source.
    pub end_trim_ms: f64,
    /// The duration of the source.
    pub source_duration_ms: f64,
}

/// The group a switch track follows.
#[derive(Debug, Serialize, JsonSchema)]
pub struct TrackSwitchView<'a> {
    /// The group type code: 0 switch group, 1 state group.
    pub group_type: u8,
    /// The kind of the group, for text.
    #[serde(skip)]
    pub group_kind: MusicGroupKind,
    /// The ID of the switch or state group.
    pub group_id: u32,
    /// The ID of the switch played when none is set.
    pub default_switch: u32,
    /// The IDs of the switches that select a clip, in clip order.
    pub switches: &'a [u32],
}

/// A music switch container.
#[derive(Debug, Serialize, JsonSchema)]
pub struct SwitchDetail<'a> {
    /// The number of transition rules.
    pub rule_count: u32,
    /// The IDs of the state or switch groups the decision tree tests, outermost first.
    pub group_ids: &'a [u32],
    /// The IDs of the nodes the leaves of the decision tree play, in tree order, 0 left out.
    pub targets: &'a [u32],
}

/// A music playlist.
#[derive(Debug, Serialize, JsonSchema)]
pub struct PlaylistDetail {
    /// The number of transition rules.
    pub rule_count: u32,
    /// The items in depth-first order.
    pub items: Vec<ItemView>,
}

/// An item of a playlist.
#[derive(Debug, Serialize, JsonSchema)]
pub struct ItemView {
    /// The nesting depth, 0 for the top group.
    pub depth: usize,
    /// The item ID.
    pub item_id: u32,
    /// The segment, 0 for a group.
    pub segment_id: u32,
    /// How many child items follow.
    pub children: u32,
    /// The play mode code: 0 random, 1 sequence continuous, 2 random step, 3 sequence step.
    pub play_mode: u32,
    /// The play mode, for text.
    #[serde(skip)]
    pub mode: MusicPlayMode,
    /// How often the item repeats, 0 for endlessly.
    pub loops: i16,
    /// The least random repeat count.
    pub loop_min: i16,
    /// The most random repeat count.
    pub loop_max: i16,
    /// The weight among its siblings.
    pub weight: u32,
    /// How many plays pass before the item may repeat.
    pub avoid_repeat_count: u16,
    /// Whether the weight is used.
    pub uses_weight: bool,
    /// Whether the children play in shuffled order.
    pub shuffle: bool,
}

impl<'a> ObjectView<'a> {
    pub(crate) fn attach_source_fields(&mut self, bank: &'a Bank) {
        self.source_fields = bank
            .soundbank()
            .objects()
            .get(self.index)
            .and_then(|object| ObjectSourceFields::new(bank, self.index, object.kind));
    }

    /// Describes the object at `index` in bank order.
    #[must_use]
    pub fn new(index: usize, object: &'a HircObject) -> Self {
        Self {
            index,
            id: object.id,
            kind: object.kind.slug(),
            type_byte: match object.kind {
                HircKind::Other(byte) => Some(byte),
                _ => None,
            },
            layout: object.layout(),
            error: object.error().map(ToString::to_string),
            parent: object.parent_id().filter(|parent| *parent != 0),
            children: object.children().collect(),
            detail: detail(&object.value),
            source_fields: None,
        }
    }
}

/// The `status` of an object in JSON: [`HircLayout`] by name.
#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Status {
    Decoded,
    Undecoded,
    Unsupported,
    Malformed,
}

impl From<HircLayout> for Status {
    fn from(layout: HircLayout) -> Self {
        match layout {
            HircLayout::Decoded => Self::Decoded,
            HircLayout::Undecoded => Self::Undecoded,
            HircLayout::Unsupported => Self::Unsupported,
            HircLayout::Malformed => Self::Malformed,
        }
    }
}

#[expect(
    clippy::trivially_copy_pass_by_ref,
    reason = "serde's serialize_with passes the field by reference"
)]
fn serialize_layout<S: Serializer>(layout: &HircLayout, serializer: S) -> Result<S::Ok, S::Error> {
    Status::from(*layout).serialize(serializer)
}
