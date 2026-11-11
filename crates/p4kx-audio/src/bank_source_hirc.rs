// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::SeekObjectFields;
use crate::bank_reader::SectionReader;
use crate::bank_source_container::{self, BankHircContainer};
use crate::bank_source_event;
use crate::bank_source_music_segment::{self, BankHircMusicSegment};
use crate::bank_source_sound::{self, BankHircSound};
use crate::bank_structure::{HircEnvelope, OBJECT_HEADER_LEN, OBJECT_ID_LEN};
use crate::hirc::{self, CONTAINER_VERSION};
use crate::hirc_action;
use crate::hirc_container::Layout as ContainerLayout;
use crate::hirc_source;
use crate::read_at::SeekSource;
use crate::{
    ActionTarget, BankActionFields, BankError, BankHircEvent, BankObservedHeader, BankReadError,
    BankSourceSection, HircKind, SectionId,
};

const COUNT_LEN: u64 = size_of::<u32>() as u64;

/// The raw header and ID of one original physical HIRC object.
#[derive(Debug, Clone, Copy)]
pub struct BankHircObject {
    ordinal: u64,
    section_ordinal: u64,
    header_offset: u64,
    body_len: u64,
    type_code: u8,
    id: u32,
}

impl BankHircObject {
    /// Returns the zero-based occurrence position in its HIRC section.
    #[must_use]
    pub const fn ordinal(&self) -> u64 {
        self.ordinal
    }

    /// Returns the original physical HIRC section position.
    #[must_use]
    pub const fn section_ordinal(&self) -> u64 {
        self.section_ordinal
    }

    /// Returns the bank-relative byte offset of the object's type byte.
    #[must_use]
    pub const fn header_offset(&self) -> u64 {
        self.header_offset
    }

    /// Returns the bank-relative byte offset of the body, beginning with the object ID.
    #[must_use]
    pub const fn body_offset(&self) -> u64 {
        self.header_offset + OBJECT_HEADER_LEN as u64
    }

    /// Returns the original body length, including the ID.
    #[must_use]
    pub const fn body_len(&self) -> u64 {
        self.body_len
    }

    /// Returns the exclusive bank-relative end of the object body.
    #[must_use]
    pub const fn body_end_exclusive(&self) -> u64 {
        self.body_offset() + self.body_len
    }

    /// Returns the original authored object-type byte.
    #[must_use]
    pub const fn type_code(&self) -> u8 {
        self.type_code
    }

    /// Returns the original object ID word.
    #[must_use]
    pub const fn id(&self) -> u32 {
        self.id
    }

    /// Interprets the type byte using an actual bank header version.
    #[must_use]
    pub const fn kind(&self, version: u32) -> HircKind {
        HircKind::of(version, self.type_code)
    }
}

/// An original HIRC occurrence and the Action interpretation made from that same read.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankHircAction {
    /// Original object envelope and ID.
    pub object: BankHircObject,
    /// The Action interpretation or the reason it was unavailable.
    pub status: BankActionRead,
}

/// The interpretation of an original object as an Action.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankActionRead {
    /// The physical type is another HIRC kind.
    NotAction,
    /// Action semantics were decoded.
    Decoded {
        /// The same Action value used by the slice reader.
        target: ActionTarget,
        /// The original reads behind that value.
        fields: BankActionFields,
    },
    /// The known layout could not be decoded completely.
    Malformed {
        /// The format failure, with extents relative to the object body.
        error: BankError,
        /// The original fields read before the failure.
        fields: BankActionFields,
    },
    /// The Action layout is unknown for the observed bank version.
    Unsupported {
        /// The unsupported layout and bank version.
        error: BankError,
    },
}

/// A physical HIRC occurrence and any selected bounded interpretation.
#[non_exhaustive]
pub enum BankHircSemantic<'input, R> {
    /// Physical object whose semantic fields are outside this iterator's coverage.
    Raw(BankHircObject),
    /// Owner-bound Action interpretation.
    Action(BankHircAction),
    /// Owner-bound Event interpretation.
    Event(BankHircEvent<'input, R>),
    /// Owner-bound Sound interpretation.
    Sound(Box<BankHircSound>),
    /// Owner-bound container interpretation and ordered children.
    Container(Box<BankHircContainer<'input, R>>),
    /// Owner-bound music segment interpretation and ordered values.
    MusicSegment(Box<BankHircMusicSegment<'input, R>>),
}

impl<R> BankHircSemantic<'_, R> {
    /// Returns the original physical object envelope.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        match self {
            Self::Raw(object) => object,
            Self::Action(action) => &action.object,
            Self::Event(event) => &event.object,
            Self::Sound(sound) => &sound.object,
            Self::Container(container) => &container.object,
            Self::MusicSegment(segment) => &segment.object,
        }
    }
}

/// Fixed-memory iterator over the physical HIRC objects of one section.
pub struct BankHirc<'input, R> {
    input: &'input mut SeekSource<R>,
    section: BankSourceSection,
    position: u64,
    count: u32,
    ordinal: u32,
    stopped: bool,
}

impl<'input, R: Read + Seek> BankHirc<'input, R> {
    pub(crate) fn open(
        input: &'input mut SeekSource<R>,
        section: &BankSourceSection,
    ) -> Result<Self, BankReadError> {
        let mut count = [0; size_of::<u32>()];
        if section.body_len() < COUNT_LEN {
            return Err(truncated(section, COUNT_LEN).into());
        }
        input.read_io_at(section.body_offset(), &mut count)?;
        Ok(Self {
            input,
            section: section.clone(),
            position: section.body_offset() + COUNT_LEN,
            count: u32::from_le_bytes(count),
            ordinal: 0,
            stopped: false,
        })
    }
}

impl<R> BankHirc<'_, R> {
    /// Returns the authored object count.
    #[must_use]
    pub const fn object_count(&self) -> u32 {
        self.count
    }

    /// Returns the exact trailing range after a complete object scan, including an empty range.
    #[must_use]
    pub const fn trailing_range(&self) -> Option<Range<u64>> {
        if self.stopped || self.ordinal != self.count {
            None
        } else {
            Some(self.position..self.section.body_end_exclusive())
        }
    }
}

impl<R: Read + Seek> Iterator for BankHirc<'_, R> {
    type Item = Result<BankHircObject, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.stopped || self.ordinal == self.count {
            return None;
        }
        let result = self.object();
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }
}

impl<R: Read + Seek> BankHirc<'_, R> {
    /// Reads the next physical object and decodes Action fields from its admitted body.
    ///
    /// The caller chooses the applicable BKHD occurrence from this same source.
    pub fn next_action(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircAction, BankReadError>> {
        let version = match self.observed_version(header) {
            Ok(version) => version,
            Err(cause) => return Some(Err(cause)),
        };
        let object = match self.next()? {
            Ok(object) => object,
            Err(error) => return Some(Err(error)),
        };
        let result = self.action(object, version);
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }

    /// Reads the next physical occurrence and its supported semantic fields.
    ///
    /// The applicable BKHD observation must belong to this source.
    pub fn next_semantic(
        &mut self,
        header: &BankObservedHeader,
    ) -> Option<Result<BankHircSemantic<'_, R>, BankReadError>> {
        let version = match self.observed_version(header) {
            Ok(version) => version,
            Err(cause) => return Some(Err(cause)),
        };
        let object = match self.next()? {
            Ok(object) => object,
            Err(cause) => return Some(Err(cause)),
        };
        let kind = object.kind(version);
        if version >= CONTAINER_VERSION && kind == HircKind::MusicSegment {
            return Some(
                bank_source_music_segment::observe(self.input, &mut self.stopped, object, version)
                    .map(|segment| BankHircSemantic::MusicSegment(Box::new(segment))),
            );
        }
        if version >= CONTAINER_VERSION
            && let Some(layout) = ContainerLayout::of(kind)
        {
            return Some(
                bank_source_container::observe(
                    self.input,
                    &mut self.stopped,
                    object,
                    version,
                    kind,
                    layout,
                )
                .map(|container| BankHircSemantic::Container(Box::new(container))),
            );
        }
        let result = match kind {
            HircKind::Event => {
                return Some(
                    bank_source_event::observe(self.input, &mut self.stopped, object, version)
                        .map(BankHircSemantic::Event),
                );
            }
            HircKind::Action => self.action(object, version).map(BankHircSemantic::Action),
            HircKind::Sound if hirc_source::has_sound_layout(version) => {
                bank_source_sound::observe(self.input, object, version)
                    .map(|sound| BankHircSemantic::Sound(Box::new(sound)))
            }
            _ => Ok(BankHircSemantic::Raw(object)),
        };
        if result.is_err() {
            self.stopped = true;
        }
        Some(result)
    }

    fn observed_version(&self, header: &BankObservedHeader) -> Result<u32, BankReadError> {
        header
            .section()
            .require(&self.section.owner, SectionId::BKHD)?;
        let version = header.header().version;
        if header.header().is_supported_version() {
            Ok(version)
        } else {
            Err(BankError::UnsupportedVersion { version }.into())
        }
    }

    fn action(
        &mut self,
        object: BankHircObject,
        version: u32,
    ) -> Result<BankHircAction, BankReadError> {
        let kind = object.kind(version);
        let status = if kind != HircKind::Action {
            BankActionRead::NotAction
        } else if hirc::has_unknown_layout(kind, version) {
            BankActionRead::Unsupported {
                error: BankError::UnsupportedObjectLayout { kind, version },
            }
        } else {
            let mut reader = SeekObjectFields::after_id(
                self.input,
                object.body_offset(),
                object.body_len(),
                OBJECT_ID_LEN as u64,
            );
            let mut fields = BankActionFields::default();
            match hirc_action::read(version, &mut reader, &mut fields) {
                Ok(target) => BankActionRead::Decoded { target, fields },
                Err(BankReadError::Format(error)) => BankActionRead::Malformed { error, fields },
                Err(error) => return Err(error),
            }
        };
        Ok(BankHircAction { object, status })
    }

    fn object(&mut self) -> Result<BankHircObject, BankReadError> {
        let relative = self.position - self.section.body_offset();
        let remaining = self.section.body_len() - relative;
        if remaining < OBJECT_HEADER_LEN as u64 {
            let needed = if remaining == 0 {
                1
            } else {
                OBJECT_HEADER_LEN as u64
            };
            return Err(truncated(&self.section, relative + needed).into());
        }
        let mut header = [0; OBJECT_HEADER_LEN];
        self.input.read_io_at(self.position, &mut header)?;
        let header = HircEnvelope::read(&mut SectionReader::new(SectionId::HIRC, &header))?;
        let extent = OBJECT_HEADER_LEN as u64 + u64::from(header.body_len);
        if extent > remaining {
            return Err(truncated(&self.section, relative + extent).into());
        }
        let mut id = [0; OBJECT_ID_LEN];
        self.input
            .read_io_at(self.position + OBJECT_HEADER_LEN as u64, &mut id)?;
        let result = BankHircObject {
            ordinal: u64::from(self.ordinal),
            section_ordinal: self.section.ordinal(),
            header_offset: self.position,
            body_len: u64::from(header.body_len),
            type_code: header.type_code,
            id: u32::from_le_bytes(id),
        };
        self.position = result.body_end_exclusive();
        self.ordinal += 1;
        Ok(result)
    }
}

const fn truncated(section: &BankSourceSection, needed: u64) -> BankError {
    BankError::Truncated {
        section: SectionId::HIRC,
        len: section.body_len(),
        needed,
    }
}
