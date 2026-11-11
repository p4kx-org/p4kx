// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_music_track_automation_rows::BankMusicTrackAutomationRows;
use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc::CONTAINER_VERSION;
use crate::hirc_music_track::{self, BankMusicTrackFields, BankMusicTrackRow};
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError, HircKind, SectionId};

/// A physical `MusicTrack` and its checked original field and row data.
#[non_exhaustive]
pub struct BankHircMusicTrack<'input, R> {
    /// Original HIRC envelope.
    pub object: BankHircObject,
    /// Result of the selected layout.
    pub status: BankMusicTrackRead,
    /// Successfully observed header fields.
    pub fields: BankMusicTrackFields,
    /// Fixed-memory replay of admitted rows in physical order.
    pub rows: BankMusicTrackRows<'input, R>,
}

impl<R> BankHircMusicTrack<'_, R> {
    /// Borrows checked original automation rows in physical order.
    pub fn automations(&mut self) -> Option<BankMusicTrackAutomationRows<'_, R>> {
        let extent = self.fields.automation_extent.as_ref()?;
        let count = self.fields.automation_count.as_ref()?.value;
        Some(BankMusicTrackAutomationRows::new(
            self.rows.input,
            self.rows.stopped,
            &extent.checked_range,
            count,
        ))
    }
}

/// Interpretation status of a physical `MusicTrack`.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankMusicTrackRead {
    /// Existing-reader layout fits the original body.
    Decoded,
    /// The selected layout failed after an original prefix.
    Malformed(BankError),
    /// The bank version has no known `MusicTrack` layout.
    Unsupported(BankError),
}

#[derive(Clone, Copy, Default)]
struct List {
    start: u64,
    end: u64,
    count: u32,
}

impl List {
    const fn record(&mut self, range: &Range<u64>) {
        if self.count == 0 {
            self.start = range.start;
        }
        self.end = range.end;
        self.count += 1;
    }
}

#[derive(Default)]
struct Lists {
    sources: List,
    clips: List,
    switches: List,
}

impl Lists {
    const fn record(&mut self, row: &BankMusicTrackRow) {
        match row {
            BankMusicTrackRow::Source { value, .. } => self.sources.record(&value.range),
            BankMusicTrackRow::Clip { value, .. } => self.clips.record(&value.range),
            BankMusicTrackRow::Switch { id, .. } => self.switches.record(&id.range),
        }
    }
}

#[derive(Clone, Copy)]
enum Phase {
    Source,
    Clip,
    Switch,
    End,
}

/// Bounded replay of the admitted source, clip and switch list prefixes.
pub struct BankMusicTrackRows<'input, R> {
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    lists: Lists,
    phase: Phase,
    ordinal: u32,
    position: u64,
}

impl<'input, R> BankMusicTrackRows<'input, R> {
    const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        lists: Lists,
    ) -> Self {
        Self {
            input,
            stopped,
            lists,
            phase: Phase::Source,
            ordinal: 0,
            position: 0,
        }
    }
}

impl<R: Read + Seek> BankMusicTrackRows<'_, R> {
    fn row(&mut self, list: List) -> Result<BankMusicTrackRow, BankReadError> {
        let ordinal = self.ordinal;
        let mut input = SeekObjectFields::after_id(
            self.input,
            list.start,
            list.end - list.start,
            self.position,
        );
        let value = match self.phase {
            Phase::Source => BankMusicTrackRow::Source {
                ordinal,
                value: hirc_music_track::source(CONTAINER_VERSION, &mut input)?,
            },
            Phase::Clip => BankMusicTrackRow::Clip {
                ordinal,
                value: hirc_music_track::clip(CONTAINER_VERSION, &mut input)?,
            },
            Phase::Switch => BankMusicTrackRow::Switch {
                ordinal,
                id: input.read_u32()?,
            },
            Phase::End => {
                return Err(BankError::invalid(
                    SectionId::HIRC,
                    "the track list cursor is exhausted",
                )
                .into());
            }
        };
        self.position = input.offset() - list.start;
        Ok(value)
    }
}

impl<R: Read + Seek> Iterator for BankMusicTrackRows<'_, R> {
    type Item = Result<BankMusicTrackRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped {
            return None;
        }
        loop {
            let (list, following) = match self.phase {
                Phase::Source => (self.lists.sources, Phase::Clip),
                Phase::Clip => (self.lists.clips, Phase::Switch),
                Phase::Switch => (self.lists.switches, Phase::End),
                Phase::End => return None,
            };
            if self.ordinal == list.count {
                self.phase = following;
                self.ordinal = 0;
                self.position = 0;
                continue;
            }
            let row = self.row(list);
            if row.is_ok() {
                self.ordinal += 1;
            } else {
                *self.stopped = true;
            }
            return Some(row);
        }
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    version: u32,
) -> Result<BankHircMusicTrack<'input, R>, BankReadError> {
    let mut fields = BankMusicTrackFields::default();
    let mut lists = Lists::default();
    let status = if version == CONTAINER_VERSION {
        let mut reader = SeekObjectFields::after_id(
            input,
            object.body_offset(),
            object.body_len(),
            OBJECT_ID_LEN as u64,
        );
        match hirc_music_track::read(version, &mut reader, &mut fields, |row| lists.record(&row)) {
            Ok(_) => BankMusicTrackRead::Decoded,
            Err(BankReadError::Format(error)) => BankMusicTrackRead::Malformed(error),
            Err(error) => {
                *stopped = true;
                return Err(error);
            }
        }
    } else {
        BankMusicTrackRead::Unsupported(BankError::UnsupportedObjectLayout {
            kind: HircKind::MusicTrack,
            version,
        })
    };
    Ok(BankHircMusicTrack {
        object,
        status,
        fields,
        rows: BankMusicTrackRows::new(input, stopped, lists),
    })
}
