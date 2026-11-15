// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircMusicPlaylist, BankHircMusicSegment, BankHircMusicSwitch, BankHircObject,
    BankMusicNodeFields, BankMusicPlaylistRead, BankMusicPlaylistRow, BankMusicSegmentRead,
    BankMusicSegmentRow, BankMusicSwitchRead, BankMusicSwitchRow, BankObservedHeader,
    BankReadError, HircKind,
};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

#[path = "hirc_music_stingers.rs"]
mod stingers;

use self::stingers::{Observation, Stinger, Stingers};
use super::origins::ObjectOrigins;

pub(in crate::hirc_view) type MusicNodeSourceSchema =
    MusicNodeSourceDocument<Vec<Stinger>, SourceInventory>;

#[derive(Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "MusicNodeObjectSourceFields")]
pub(in crate::hirc_view) struct MusicNodeSourceDocument<Rows, Inventory> {
    kind: SourceKind,
    layout: SourcedValue<Layout>,
    version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SourcedValue<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stinger_count: Option<SourcedValue<u32>>,
    #[schemars(with = "Vec<Stinger>")]
    stingers: Rows,
    #[schemars(with = "SourceInventory")]
    source_inventory: Inventory,
}

#[derive(Clone, Copy, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum SourceKind {
    #[serde(rename = "music-playlist")]
    Playlist,
    #[serde(rename = "music-segment")]
    Segment,
    #[serde(rename = "music-switch")]
    Switch,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum Layout {
    Decoded,
    Malformed,
    Unsupported,
}

pub(super) fn playlist<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    node: BankHircMusicPlaylist<'_, R>,
) -> Result<S::Ok, S::Error> {
    use BankMusicPlaylistRead::{Decoded, Malformed, Unsupported};

    let disposition = match node.status {
        Decoded => (Layout::Decoded, None),
        Malformed(error) => (Layout::Malformed, Some(error.to_string())),
        Unsupported(_) => (Layout::Unsupported, None),
        _ => {
            return Err(S::Error::custom(
                "unexpected original music-node disposition",
            ));
        }
    };
    let site = DocumentSite {
        kind: SourceKind::Playlist,
        origins,
        header,
        original: &node.object,
    };
    observed(
        serializer,
        &site,
        disposition,
        node.fields.node,
        node.rows,
        |row| match row {
            BankMusicPlaylistRow::Child { .. } => Ok(Observation::Skip),
            BankMusicPlaylistRow::Stinger { ordinal, value } => {
                Ok(Observation::Stinger { ordinal, value })
            }
            BankMusicPlaylistRow::Item { .. } => Ok(Observation::End),
            _ => Err("unexpected original music-node row"),
        },
    )
}

pub(super) fn segment<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    node: BankHircMusicSegment<'_, R>,
) -> Result<S::Ok, S::Error> {
    use BankMusicSegmentRead::{Decoded, Malformed, Unsupported};

    let disposition = match node.status {
        Decoded => (Layout::Decoded, None),
        Malformed(error) => (Layout::Malformed, Some(error.to_string())),
        Unsupported(_) => (Layout::Unsupported, None),
        _ => {
            return Err(S::Error::custom(
                "unexpected original music-node disposition",
            ));
        }
    };
    let site = DocumentSite {
        kind: SourceKind::Segment,
        origins,
        header,
        original: &node.object,
    };
    observed(
        serializer,
        &site,
        disposition,
        node.fields.node,
        node.rows,
        |row| match row {
            BankMusicSegmentRow::Child { .. } => Ok(Observation::Skip),
            BankMusicSegmentRow::Stinger { ordinal, value } => {
                Ok(Observation::Stinger { ordinal, value })
            }
            BankMusicSegmentRow::Marker { .. } => Ok(Observation::End),
            _ => Err("unexpected original music-node row"),
        },
    )
}

pub(super) fn switch<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    node: BankHircMusicSwitch<'_, R>,
) -> Result<S::Ok, S::Error> {
    use BankMusicSwitchRead::{Decoded, Malformed, Unsupported};

    let disposition = match node.status {
        Decoded => (Layout::Decoded, None),
        Malformed(error) => (Layout::Malformed, Some(error.to_string())),
        Unsupported(_) => (Layout::Unsupported, None),
        _ => {
            return Err(S::Error::custom(
                "unexpected original music-node disposition",
            ));
        }
    };
    let site = DocumentSite {
        kind: SourceKind::Switch,
        origins,
        header,
        original: &node.object,
    };
    observed(
        serializer,
        &site,
        disposition,
        node.fields.node,
        node.rows,
        |row| match row {
            BankMusicSwitchRow::Child { .. } => Ok(Observation::Skip),
            BankMusicSwitchRow::Stinger { ordinal, value } => {
                Ok(Observation::Stinger { ordinal, value })
            }
            BankMusicSwitchRow::Group { .. }
            | BankMusicSwitchRow::Node { .. }
            | BankMusicSwitchRow::Target { .. } => Ok(Observation::End),
            _ => Err("unexpected original music-node row"),
        },
    )
}

fn observed<S, I, F, Row>(
    serializer: S,
    site: &DocumentSite<'_>,
    (layout, error): (Layout, Option<String>),
    fields: BankMusicNodeFields,
    input: I,
    select: F,
) -> Result<S::Ok, S::Error>
where
    S: Serializer,
    I: Iterator<Item = Result<Row, BankReadError>>,
    F: Fn(Row) -> Result<Observation, &'static str>,
{
    let stinger_count = site
        .origins
        .optional(fields.stinger_count.as_ref(), "musicStingerCount")
        .map_err(S::Error::custom)?;
    let rows = Stingers::new(input, select, fields.stinger_tail, site.origins);
    site.write(serializer, layout, error, stinger_count, rows)
}

pub(super) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    original: &BankHircObject,
    kind: HircKind,
) -> Result<S::Ok, S::Error> {
    let kind = match kind {
        HircKind::MusicPlaylist => SourceKind::Playlist,
        HircKind::MusicSegment => SourceKind::Segment,
        HircKind::MusicSwitch => SourceKind::Switch,
        _ => return Err(S::Error::custom("not an original music node")),
    };
    DocumentSite {
        kind,
        origins,
        header,
        original,
    }
    .write(
        serializer,
        Layout::Unsupported,
        None,
        None,
        [] as [Stinger; 0],
    )
}

struct DocumentSite<'source> {
    kind: SourceKind,
    origins: &'source ObjectOrigins,
    header: &'source BankObservedHeader,
    original: &'source BankHircObject,
}

impl DocumentSite<'_> {
    fn write<S: Serializer, Rows: Serialize>(
        &self,
        serializer: S,
        layout: Layout,
        error: Option<String>,
        stinger_count: Option<SourcedValue<u32>>,
        stingers: Rows,
    ) -> Result<S::Ok, S::Error> {
        MusicNodeSourceDocument {
            kind: self.kind,
            layout: self
                .origins
                .coverage(layout, self.header, self.original)
                .map_err(S::Error::custom)?,
            version: self
                .origins
                .version(self.header)
                .map_err(S::Error::custom)?,
            error: error
                .map(|error| self.origins.coverage(error, self.header, self.original))
                .transpose()
                .map_err(S::Error::custom)?,
            stinger_count,
            stingers,
            source_inventory: self.origins,
        }
        .serialize(serializer)
    }
}
