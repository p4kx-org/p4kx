// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::bank::{
    BankHircField, BankHircObject, BankMusicMarker, BankMusicNodeFields, BankMusicSegmentFields,
    BankMusicStinger,
};

use crate::audio_bank_hirc_origins::ObjectSites;
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_origins::{BankOrigins, EvidenceSite};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaWriter, Subresource};

#[path = "audio_bank_music_segment_origin_words.rs"]
mod words;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum List {
    Children,
    Stingers,
    Markers,
}

pub(crate) enum MusicChildField {
    Track,
    Child,
}

impl MusicChildField {
    const fn names(self) -> (&'static str, &'static str) {
        match self {
            Self::Track => ("track_id", "musicTrackId"),
            Self::Child => ("child_id", "musicChildId"),
        }
    }
}

pub(crate) struct MusicSites<'a> {
    object: ObjectSites,
    version: [u64; 1],
    node_fields: &'a BankMusicNodeFields,
    segment: Option<&'a BankMusicSegmentFields>,
    namespace: &'static str,
    node: Option<[u64; 2]>,
    children: Option<[u64; 2]>,
    stingers: Option<[u64; 2]>,
    markers: Option<[u64; 2]>,
}

impl<'a> MusicSites<'a> {
    pub(super) fn new(
        object: &BankHircObject,
        version: VersionSite,
        fields: &'a BankMusicSegmentFields,
    ) -> Self {
        let mut sites = Self::for_node(object, version, &fields.node, "bnkHircMusicSegment");
        sites.segment = Some(fields);
        sites.markers = fields.marker_count.as_ref().map(word);
        sites
    }

    pub(crate) fn for_node(
        object: &BankHircObject,
        version: VersionSite,
        node_fields: &'a BankMusicNodeFields,
        namespace: &'static str,
    ) -> Self {
        Self {
            object: ObjectSites::new(object),
            version: [version.body_offset],
            node_fields,
            segment: None,
            namespace,
            node: node_fields.node_range.as_ref().map(coordinates),
            children: node_fields.child_count.as_ref().map(word),
            stingers: node_fields.stinger_count.as_ref().map(word),
            markers: None,
        }
    }

    pub(crate) fn prepare_node_header<'source, 'writer, 'run>(
        &self,
        input: &'source MediaOriginInput,
        writer: &'writer MediaWriter<'run>,
    ) -> Result<BankOrigins<'source, 'writer, 'run>, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let position = prepared.capture([self.object.route()])?;
        self.object.indices_origin(&mut prepared, position)?;
        self.parent(&mut prepared)?;
        self.node_words(&mut prepared)?;
        Ok(prepared)
    }

    pub(crate) fn after_node(&self) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.list_route(List::Stingers)
    }

    pub(crate) const fn object_indices(&self) -> [u64; 2] {
        self.object.indices()
    }

    pub(crate) const fn indices(&self, ordinal: u64) -> [u64; 3] {
        let [section, object] = self.object_indices();
        [section, object, ordinal]
    }

    pub(crate) fn subresource(&self, namespace: &'static str, ordinal: u64) -> Subresource {
        Subresource::new(namespace, self.indices(ordinal).to_vec())
    }

    pub(super) fn header(
        &self,
        input: &MediaOriginInput,
        has_issue: bool,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = self.prepare_node_header(input, writer)?;
        self.segment_words(&mut prepared)?;
        let layout = prepared.capture(self.list_route(List::Markers))?;
        prepared.field("/data/payload/layout", layout.clone())?;
        if has_issue {
            prepared.subtree("/issues", layout)?;
        }
        Ok(prepared.finish())
    }

    fn parent(&self, prepared: &mut BankOrigins<'_, '_, '_>) -> Result<(), MediaError> {
        let source = &self.node_fields.parent;
        let effects = source.effect_count.as_ref().map(word);
        let metadata = source.metadata_count.as_ref().map(word);
        let parent = source.parent_id.as_ref().map(word);
        let actual = prepared.capture(
            self.base()
                .into_iter()
                .chain(
                    effects
                        .as_ref()
                        .map(|span| field(span, "musicEffectCount", SourceUseRole::Selector)),
                )
                .chain(
                    metadata
                        .as_ref()
                        .map(|span| field(span, "musicMetadataCount", SourceUseRole::Selector)),
                )
                .chain(
                    parent
                        .as_ref()
                        .map(|span| field(span, "musicParentId", SourceUseRole::Payload)),
                ),
        )?;
        prepared.field("/data/payload/parent_id", actual)
    }

    pub(crate) fn child(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        id: &Range<u64>,
        target: MusicChildField,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let position = self.row(&mut prepared, List::Children, ordinal)?;
        prepared.range("/data/payload/id_range", &position, &position)?;
        let (key, name) = target.names();
        self.row_word(&mut prepared, List::Children, ordinal, key, name, id)?;
        Ok(prepared.finish())
    }

    pub(crate) fn stinger(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        stinger: &BankMusicStinger,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let position = self.row(&mut prepared, List::Stingers, ordinal)?;
        prepared.range("/data/payload/opaque_range", &position, &position)?;
        for (key, name, range) in [
            (
                "trigger_id",
                "musicStingerTriggerId",
                &stinger.trigger_id.range,
            ),
            (
                "segment_id",
                "musicStingerSegmentId",
                &stinger.segment_id.range,
            ),
        ] {
            self.row_word(&mut prepared, List::Stingers, ordinal, key, name, range)?;
        }
        Ok(prepared.finish())
    }

    pub(super) fn marker(
        &self,
        input: &MediaOriginInput,
        ordinal: u64,
        marker: &BankMusicMarker,
        writer: &MediaWriter<'_>,
    ) -> Result<PreparedRecordOrigins, MediaError> {
        let mut prepared = BankOrigins::new(input, writer)?;
        let position = self.row(&mut prepared, List::Markers, ordinal)?;
        self.row_word(
            &mut prepared,
            List::Markers,
            ordinal,
            "marker_id",
            "musicMarkerId",
            &marker.id.range,
        )?;
        self.row_word(
            &mut prepared,
            List::Markers,
            ordinal,
            "position",
            "musicMarkerPosition",
            &marker.position.range,
        )?;
        self.marker_name(&mut prepared, ordinal, marker, &position)?;
        Ok(prepared.finish())
    }

    fn marker_name(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        ordinal: u64,
        marker: &BankMusicMarker,
        position: &OriginContributions,
    ) -> Result<(), MediaError> {
        let indices = self.indices(ordinal);
        let name = coordinates(&marker.name_range);
        let through_nul = [marker.name_range.start, marker.terminator + 1];
        let routing = self.list_route(List::Markers).chain([row_route(
            self.namespace,
            List::Markers,
            &indices,
        )]);
        let terminator = field(
            &through_nul,
            "musicMarkerTerminatedName",
            SourceUseRole::Selector,
        );
        let end = prepared.capture(routing.clone().chain([terminator]))?;
        prepared.range("/data/payload/name_range", position, &end)?;
        prepared.field("/data/payload/terminator_offset", end)?;
        let text = prepared.capture(routing.chain([
            terminator,
            field(&name, "musicMarkerName", SourceUseRole::Conversion),
        ]))?;
        prepared.field("/data/payload/name", text.clone())?;
        prepared.field("/data/payload/encoding", text)
    }

    fn row(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        list: List,
        ordinal: u64,
    ) -> Result<OriginContributions, MediaError> {
        let indices = self.indices(ordinal);
        let routing = self
            .list_route(list)
            .chain([row_route(self.namespace, list, &indices)]);
        let position = prepared.capture(routing)?;
        self.row_identity(prepared, &position)?;
        Ok(position)
    }

    pub(crate) fn row_identity(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        position: &OriginContributions,
    ) -> Result<(), MediaError> {
        self.object.indices_origin(prepared, position.clone())?;
        let object = prepared.capture([self.object.route()])?;
        prepared.field("/subresource/indices/1", object)?;
        prepared.field("/data/payload/ordinal", position.clone())
    }

    fn row_word(
        &self,
        prepared: &mut BankOrigins<'_, '_, '_>,
        list: List,
        ordinal: u64,
        key: &str,
        name: &'static str,
        range: &Range<u64>,
    ) -> Result<(), MediaError> {
        let span = coordinates(range);
        let indices = self.indices(ordinal);
        let routing = self
            .list_route(list)
            .chain([row_route(self.namespace, list, &indices)]);
        let evidence =
            prepared.capture(routing.chain([field(&span, name, SourceUseRole::Payload)]))?;
        prepared.subtree(&format!("/data/payload/{key}"), evidence)
    }

    const fn base(&self) -> [EvidenceSite<'_>; 5] {
        let [route, selection, kind, version] = self.object.versioned(&self.version);
        [
            route,
            selection,
            kind,
            version,
            self.object.extent(SourceUseRole::Selector),
        ]
    }

    fn preceding(&self, list: Option<List>) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.base()
            .into_iter()
            .chain(
                self.node
                    .as_ref()
                    .map(|range| field(range, "musicNodeLayout", SourceUseRole::Selector)),
            )
            .chain(
                (list >= Some(List::Children))
                    .then_some(self.children.as_ref())
                    .flatten()
                    .map(|range| field(range, "musicChildCount", SourceUseRole::Selector)),
            )
            .chain(
                (list >= Some(List::Stingers))
                    .then_some(self.stingers.as_ref())
                    .flatten()
                    .map(|range| field(range, "musicStingerCount", SourceUseRole::Selector)),
            )
            .chain(
                (list >= Some(List::Markers))
                    .then_some(self.markers.as_ref())
                    .flatten()
                    .map(|range| field(range, "musicMarkerCount", SourceUseRole::Selector)),
            )
    }

    fn list_route(&self, list: List) -> impl Iterator<Item = EvidenceSite<'_>> + Clone {
        self.preceding(Some(list))
    }
}

const fn row_route<'a>(
    namespace: &'static str,
    list: List,
    indices: &'a [u64; 3],
) -> EvidenceSite<'a> {
    let name = match list {
        List::Children => "childRouting",
        List::Stingers => "stingerRouting",
        List::Markers => "markerPrefixRouting",
    };
    (
        OriginSite::new(namespace, indices, name),
        SourceUseRole::Selector,
    )
}

const fn word<T>(value: &BankHircField<T>) -> [u64; 2] {
    coordinates(&value.range)
}

const fn coordinates(range: &Range<u64>) -> [u64; 2] {
    [range.start, range.end]
}

const fn field<'a>(
    range: &'a [u64; 2],
    name: &'static str,
    role: SourceUseRole,
) -> EvidenceSite<'a> {
    (OriginSite::new("bnkHircField", range, name), role)
}
