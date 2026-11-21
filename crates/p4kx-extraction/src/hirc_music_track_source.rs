// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_audio::{BankHircMusicTrack, BankHircObject, BankMusicTrackRead, BankObservedHeader};
use serde::ser::Error as _;
use serde::{Serialize, Serializer};

#[path = "hirc_music_track_source_dto.rs"]
mod dto;
#[path = "hirc_music_track_source_rows.rs"]
mod rows;
#[path = "hirc_music_track_source_summary.rs"]
mod summary;

pub(super) use self::dto::MusicTrackSourceSchema;
use self::dto::{
    MusicTrackSourceDocument, MusicTrackSourceFields, MusicTrackSourceKind, MusicTrackSourceLayout,
    MusicTrackSourceRow,
};
use self::rows::MusicTrackRows;
use super::origins::ObjectOrigins;

pub(super) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    track: BankHircMusicTrack<'_, R>,
) -> Result<S::Ok, S::Error> {
    let (layout, error) = match &track.status {
        BankMusicTrackRead::Decoded => (MusicTrackSourceLayout::Decoded, None),
        BankMusicTrackRead::Malformed(error) => {
            (MusicTrackSourceLayout::Malformed, Some(error.to_string()))
        }
        BankMusicTrackRead::Unsupported(_) => {
            return unsupported(serializer, origins, header, &track.object);
        }
        _ => {
            return Err(S::Error::custom(
                "unexpected original MusicTrack disposition",
            ));
        }
    };
    let fields = summary::read(&track.fields, origins).map_err(S::Error::custom)?;
    let original = track.object;
    let document = DocumentSite {
        origins,
        header,
        original: &original,
    };
    document.serialize(
        serializer,
        layout,
        error,
        fields,
        MusicTrackRows::new(track, origins),
    )
}

pub(super) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    original: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    DocumentSite {
        origins,
        header,
        original,
    }
    .serialize(
        serializer,
        MusicTrackSourceLayout::Unsupported,
        None,
        MusicTrackSourceFields::default(),
        [] as [MusicTrackSourceRow; 0],
    )
}

struct DocumentSite<'source> {
    origins: &'source ObjectOrigins,
    header: &'source BankObservedHeader,
    original: &'source BankHircObject,
}

impl DocumentSite<'_> {
    fn serialize<S: Serializer, Rows: Serialize>(
        &self,
        serializer: S,
        layout: MusicTrackSourceLayout,
        error: Option<String>,
        fields: MusicTrackSourceFields,
        rows: Rows,
    ) -> Result<S::Ok, S::Error> {
        MusicTrackSourceDocument {
            kind: MusicTrackSourceKind::MusicTrack,
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
            fields,
            rows,
            source_inventory: self.origins,
        }
        .serialize(serializer)
    }
}
