// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc::CONTAINER_VERSION;
use crate::hirc_music_node::{self, BankMusicNodeFields, MusicList, MusicNodeLists, STINGER_LEN};
use crate::hirc_music_playlist::{self, BankMusicPlaylistItem, PLAYLIST_ITEM_LEN};
use crate::hirc_music_playlist_depth::PlaylistDepth;
use crate::hirc_music_segment;
use crate::hirc_music_transition::{self, BankMusicTransitionFields};
use crate::hirc_node::ID_LEN;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankMusicStinger, BankReadError, HircKind};

/// A physical music playlist with its original fields and admitted rows.
#[non_exhaustive]
pub struct BankHircMusicPlaylist<'input, R> {
    /// Original object envelope.
    pub object: BankHircObject,
    /// Result of selecting the original layout.
    pub status: BankMusicPlaylistRead,
    /// Successfully observed original fields.
    pub fields: BankMusicPlaylistFields,
    /// Bounded cursor over complete original rows.
    pub rows: BankMusicPlaylistRows<'input, R>,
}

/// Interpretation status of a physical music playlist.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankMusicPlaylistRead {
    /// Existing-reader fields fit the body.
    Decoded,
    /// Original layout failed after a readable prefix.
    Malformed(BankError),
    /// The bank version has no known playlist layout.
    Unsupported(BankError),
}

/// Original playlist header fields.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicPlaylistFields {
    /// Original music node fields.
    pub node: BankMusicNodeFields,
    /// Original transition rule count and opaque extent.
    pub transitions: BankMusicTransitionFields,
    /// Original item count.
    pub item_count: Option<BankHircField<u32>>,
}

/// An ordered original row of a music playlist.
#[derive(Debug)]
#[non_exhaustive]
#[expect(
    clippy::large_enum_variant,
    reason = "Original item receipts stream without per-row allocation"
)]
pub enum BankMusicPlaylistRow {
    /// One original child ID.
    Child {
        /// Original child list position.
        ordinal: u32,
        /// Original child ID and byte range.
        id: BankHircField<u32>,
    },
    /// One original stinger.
    Stinger {
        /// Original stinger list position.
        ordinal: u32,
        /// Original stinger fields and opaque extent.
        value: BankMusicStinger,
    },
    /// One original item.
    Item {
        /// Original playlist item position.
        ordinal: u32,
        /// Original item words and range.
        value: BankMusicPlaylistItem,
    },
}

#[derive(Default)]
struct Lists {
    node: MusicNodeLists,
    items: MusicList,
}

#[derive(Clone, Copy)]
enum ListKind {
    Children,
    Stingers,
    Items,
}

impl ListKind {
    const fn following(self) -> Option<Self> {
        match self {
            Self::Children => Some(Self::Stingers),
            Self::Stingers => Some(Self::Items),
            Self::Items => None,
        }
    }

    const fn row_len(self) -> usize {
        match self {
            Self::Children => ID_LEN,
            Self::Stingers => STINGER_LEN,
            Self::Items => PLAYLIST_ITEM_LEN,
        }
    }
}

impl Lists {
    const fn get(&self, kind: ListKind) -> MusicList {
        match kind {
            ListKind::Children => self.node.children,
            ListKind::Stingers => self.node.stingers,
            ListKind::Items => self.items,
        }
    }
}

/// One derived playlist depth and its original preceding child-count word geometry.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicPlaylistDepth {
    /// Nesting depth of the current item, starting at zero.
    pub value: usize,
    preceding_child_counts: Option<[u64; 4]>,
}

impl BankMusicPlaylistDepth {
    /// Returns original first offset, word count, stride and word width for preceding child counts.
    #[must_use]
    pub const fn preceding_child_counts(&self) -> Option<&[u64; 4]> {
        self.preceding_child_counts.as_ref()
    }
}

/// Bounded original row cursor with a stack proportional to item nesting.
pub struct BankMusicPlaylistRows<'input, R> {
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    lists: Lists,
    list: Option<ListKind>,
    next: u32,
    depths: PlaylistDepth,
    item_depth: Option<BankMusicPlaylistDepth>,
}

impl<R> BankMusicPlaylistRows<'_, R> {
    /// Returns the depth receipt of the current item row, when present.
    #[must_use]
    pub const fn current_item_depth(&self) -> Option<&BankMusicPlaylistDepth> {
        self.item_depth.as_ref()
    }
}

impl<R: Read + Seek> BankMusicPlaylistRows<'_, R> {
    fn read_row(
        &mut self,
        kind: ListKind,
        list: MusicList,
    ) -> Result<BankMusicPlaylistRow, BankReadError> {
        let ordinal = self.next;
        let position = u64::from(ordinal) * kind.row_len() as u64;
        let mut reader = SeekObjectFields::after_id(self.input, list.start, list.length, position);
        Ok(match kind {
            ListKind::Children => BankMusicPlaylistRow::Child {
                ordinal,
                id: reader.read_u32()?,
            },
            ListKind::Stingers => BankMusicPlaylistRow::Stinger {
                ordinal,
                value: hirc_music_node::stinger(&mut reader)?,
            },
            ListKind::Items => {
                let value = hirc_music_playlist::item(&mut reader)?;
                self.item_depth = Some(BankMusicPlaylistDepth {
                    value: self.depths.advance(value.children.value),
                    preceding_child_counts: (ordinal > 0).then_some([
                        value.children.range.start - position,
                        u64::from(ordinal),
                        PLAYLIST_ITEM_LEN as u64,
                        value.children.range.end - value.children.range.start,
                    ]),
                });
                BankMusicPlaylistRow::Item { ordinal, value }
            }
        })
    }
}

impl<R: Read + Seek> Iterator for BankMusicPlaylistRows<'_, R> {
    type Item = Result<BankMusicPlaylistRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.item_depth = None;
        if *self.stopped {
            return None;
        }
        while let Some(kind) = self.list {
            let list = self.lists.get(kind);
            if self.next == list.count {
                self.list = kind.following();
                self.next = 0;
                continue;
            }
            let row = self.read_row(kind, list);
            if row.is_ok() {
                self.next += 1;
            } else {
                *self.stopped = true;
            }
            return Some(row);
        }
        None
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
    object: BankHircObject,
    version: u32,
) -> Result<BankHircMusicPlaylist<'input, R>, BankReadError> {
    let mut fields = BankMusicPlaylistFields::default();
    let mut lists = Lists::default();
    let status = if version == CONTAINER_VERSION {
        match scan(input, &object, &mut fields, &mut lists) {
            Ok(()) => BankMusicPlaylistRead::Decoded,
            Err(BankReadError::Format(error)) => BankMusicPlaylistRead::Malformed(error),
            Err(error) => {
                *stopped = true;
                return Err(error);
            }
        }
    } else {
        BankMusicPlaylistRead::Unsupported(BankError::UnsupportedObjectLayout {
            kind: HircKind::MusicPlaylist,
            version,
        })
    };
    Ok(BankHircMusicPlaylist {
        object,
        status,
        fields,
        rows: BankMusicPlaylistRows {
            input,
            stopped,
            lists,
            list: Some(ListKind::Children),
            next: 0,
            depths: PlaylistDepth::default(),
            item_depth: None,
        },
    })
}

fn scan<R: Read + Seek>(
    input: &mut SeekSource<R>,
    object: &BankHircObject,
    fields: &mut BankMusicPlaylistFields,
    lists: &mut Lists,
) -> Result<(), BankReadError> {
    let mut reader = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    hirc_music_node::scan(&mut reader, &mut fields.node, &mut lists.node)?;
    hirc_music_transition::read(&mut reader, &mut fields.transitions)?;
    let count = reader.read_u32()?;
    let items = count.value;
    fields.item_count = Some(count);
    lists.items.start = reader.offset();
    for _ in 0..items {
        hirc_music_playlist::item(&mut reader)?;
        lists.items.record(reader.offset());
    }
    hirc_music_segment::finish(&reader)
}
