// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc::CONTAINER_VERSION;
use crate::hirc_music_node::{self, MusicList, MusicNodeLists, STINGER_LEN};
use crate::hirc_music_segment;
use crate::hirc_node::ID_LEN;
use crate::read_at::SeekSource;
use crate::{
    BankError, BankHircField, BankHircObject, BankMusicSegmentFields, BankMusicStinger,
    BankReadError, HircKind, SectionId,
};

/// A physical HIRC `MusicSegment` and its original bounded value lists.
#[non_exhaustive]
pub struct BankHircMusicSegment<'input, R> {
    /// Original physical object envelope.
    pub object: BankHircObject,
    /// Interpretation of the original layout.
    pub status: BankMusicSegmentRead,
    /// Successfully observed scalar fields.
    pub fields: BankMusicSegmentFields,
    /// Original list cursor when any list was located.
    pub rows: BankMusicSegmentRows<'input, R>,
}

/// Result of the selected original `MusicSegment` layout.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankMusicSegmentRead {
    /// All existing-reader fields fit the body.
    Decoded,
    /// Original layout failed after a readable prefix.
    Malformed(BankError),
    /// The original bank version has no known `MusicSegment` layout.
    Unsupported(BankError),
}

/// One original `MusicSegment` list element and its position in its list.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankMusicSegmentRow {
    /// One original track ID.
    Child {
        /// Original position in the child list.
        ordinal: u32,
        /// Original track ID and byte range.
        id: BankHircField<u32>,
    },
    /// One original stinger.
    Stinger {
        /// Original position in the stinger list.
        ordinal: u32,
        /// Original stinger words and opaque extent.
        value: BankMusicStinger,
    },
    /// One complete original marker.
    Marker {
        /// Original position in the marker list.
        ordinal: u32,
        /// Original marker words and name extent.
        value: BankMusicMarker,
    },
}

/// Original marker words and its zero-terminated source name.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankMusicMarker {
    /// Authored marker ID.
    pub id: BankHircField<u32>,
    /// Marker position stored as the exact binary64 word.
    pub position: BankHircField<u64>,
    /// Original name extent excluding the terminator.
    pub name_range: Range<u64>,
    /// Original position of the NUL terminator.
    pub terminator: u64,
}

#[derive(Default)]
struct Lists {
    node: MusicNodeLists,
    markers: MusicList,
}

#[derive(Clone, Copy)]
enum Phase {
    Children,
    Stingers,
    Markers,
    End,
}

/// Fixed-memory replay of the admitted prefixes in original list order.
pub struct BankMusicSegmentRows<'input, R> {
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    lists: Lists,
    phase: Phase,
    next: u32,
    marker_offset: u64,
    name: Option<Range<u64>>,
}

impl<'input, R> BankMusicSegmentRows<'input, R> {
    const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        lists: Lists,
    ) -> Self {
        Self {
            input,
            stopped,
            lists,
            phase: Phase::Children,
            next: 0,
            marker_offset: 0,
            name: None,
        }
    }
}

impl<R: Read + Seek> BankMusicSegmentRows<'_, R> {
    /// Copies the current marker name bytes into an exact-sized buffer.
    ///
    /// # Errors
    ///
    /// Returns an error outside a marker visit, for an invalid buffer size, or on input failure.
    pub fn read_current_name(&mut self, bytes: &mut [u8]) -> Result<(), BankReadError> {
        let name = self
            .name
            .as_ref()
            .filter(|range| range.end - range.start == bytes.len() as u64)
            .ok_or_else(|| {
                BankError::invalid(
                    SectionId::HIRC,
                    "the marker name buffer does not match the current marker",
                )
            })?;
        let read = self.input.read_io_at(name.start, bytes);
        if read.is_err() {
            *self.stopped = true;
        }
        read.map_err(BankReadError::Io)
    }

    fn read_row(&mut self, list: MusicList) -> Result<BankMusicSegmentRow, BankReadError> {
        let ordinal = self.next;
        match self.phase {
            Phase::Children => {
                let position = u64::from(ordinal) * ID_LEN as u64;
                let mut reader =
                    SeekObjectFields::after_id(self.input, list.start, list.length, position);
                Ok(BankMusicSegmentRow::Child {
                    ordinal,
                    id: reader.read_u32()?,
                })
            }
            Phase::Stingers => {
                let position = u64::from(ordinal) * STINGER_LEN as u64;
                let mut reader =
                    SeekObjectFields::after_id(self.input, list.start, list.length, position);
                Ok(BankMusicSegmentRow::Stinger {
                    ordinal,
                    value: hirc_music_node::stinger(&mut reader)?,
                })
            }
            Phase::Markers => {
                let mut reader = SeekObjectFields::after_id(
                    self.input,
                    list.start,
                    list.length,
                    self.marker_offset,
                );
                let value = read_marker(&mut reader)?;
                self.marker_offset = reader.offset() - list.start;
                self.name = Some(value.name_range.clone());
                Ok(BankMusicSegmentRow::Marker { ordinal, value })
            }
            Phase::End => Err(BankError::invalid(
                SectionId::HIRC,
                "the segment list cursor is exhausted",
            )
            .into()),
        }
    }
}

impl<R: Read + Seek> Iterator for BankMusicSegmentRows<'_, R> {
    type Item = Result<BankMusicSegmentRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.name = None;
        if *self.stopped {
            return None;
        }
        loop {
            let (list, following) = match self.phase {
                Phase::Children => (self.lists.node.children, Phase::Stingers),
                Phase::Stingers => (self.lists.node.stingers, Phase::Markers),
                Phase::Markers => (self.lists.markers, Phase::End),
                Phase::End => return None,
            };
            if self.next == list.count {
                self.phase = following;
                self.next = 0;
                continue;
            }
            let row = self.read_row(list);
            if row.is_ok() {
                self.next += 1;
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
) -> Result<BankHircMusicSegment<'input, R>, BankReadError> {
    let mut fields = BankMusicSegmentFields::default();
    let mut lists = Lists::default();
    let status = if version == CONTAINER_VERSION {
        match scan(input, &object, &mut fields, &mut lists) {
            Ok(()) => BankMusicSegmentRead::Decoded,
            Err(BankReadError::Format(error)) => BankMusicSegmentRead::Malformed(error),
            Err(error) => {
                *stopped = true;
                return Err(error);
            }
        }
    } else {
        BankMusicSegmentRead::Unsupported(BankError::UnsupportedObjectLayout {
            kind: HircKind::MusicSegment,
            version,
        })
    };
    Ok(BankHircMusicSegment {
        object,
        status,
        fields,
        rows: BankMusicSegmentRows::new(input, stopped, lists),
    })
}

fn scan<R: Read + Seek>(
    input: &mut SeekSource<R>,
    object: &BankHircObject,
    fields: &mut BankMusicSegmentFields,
    lists: &mut Lists,
) -> Result<(), BankReadError> {
    let mut reader = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    hirc_music_node::scan(&mut reader, &mut fields.node, &mut lists.node)?;
    let (_, markers) = hirc_music_segment::header(&mut reader, fields)?;
    lists.markers.start = reader.offset();
    for _ in 0..markers {
        read_marker(&mut reader)?;
        lists.markers.record(reader.offset());
    }
    hirc_music_segment::finish(&reader)
}

fn read_marker<R: Read + Seek>(
    reader: &mut SeekObjectFields<'_, R>,
) -> Result<BankMusicMarker, BankReadError> {
    let words = hirc_music_segment::marker(reader)?;
    let name_range = reader.read_nul_range()?;
    let terminator = name_range.end;
    Ok(BankMusicMarker {
        id: words.id,
        position: words.position,
        name_range,
        terminator,
    })
}
