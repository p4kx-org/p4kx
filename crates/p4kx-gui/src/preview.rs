// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[cfg(test)]
mod archive_change_tests;
mod audio;
mod bank_cache;
#[cfg(test)]
mod bank_cache_tests;
mod contents;
mod entry_bytes;
mod image;
mod image_provenance;
mod reading;
mod soundbank;
mod texture_info;
mod xml;
mod xml_document;

use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError};

use p4kx_extraction::Texture;

pub use audio::{AudioPreview, AudioStatus, SoundInfo, SoundStatus, is_wem_path};
pub(crate) use bank_cache::BankCache;
pub use contents::PreviewContents;
pub(crate) use contents::{HEX_ROW_LEN, printable};
pub use image::{ImagePreview, ImageStatus, MipStatus};
pub use image_provenance::{ImageHeaderProvenance, ImageMipProvenance};
#[cfg(test)]
use reading::read;
use reading::{Spawned, spawn};
pub use soundbank::{BankSound, SoundbankPreview, SoundbankStatus, is_bank_path};
pub use texture_info::{MAX_PREVIEW_PIXELS, TextureInfo};
use xml::is_cryxml;
pub use xml::{XML_SIZE_LIMIT, XmlPreview, XmlStatus};
pub use xml_document::{XML_TEXT_LIMIT, XmlDocument};

use crate::EntryIndex;
use crate::viewer::{ModelPreview, is_model_path};

/// The largest Entry, in bytes, the window previews; larger Entries are not read.
pub const PREVIEW_SIZE_LIMIT: u64 = 256 * 1024 * 1024;

/// The most bytes a preview reads from the start of an Entry; the rest is left out.
pub const PREVIEW_READ_LIMIT: usize = 1024 * 1024;

/// The tab a preview is shown in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PreviewTab {
    /// The data decoded as text, line by line.
    #[default]
    Text,
    /// The data as offsets, hexadecimal bytes and printable characters.
    Hex,
    /// A `CryXml` Entry converted to XML.
    Xml,
    /// A texture decoded as an image, one mip at a time.
    Image,
    /// The summary of the Archive's DataForge instead of the selected Entry.
    DataForge,
    /// A WEM sound with its facts and playback controls.
    Audio,
    /// The Record selected in the Record tree instead of the selected Entry.
    Record,
    /// A model shown in the 3D viewer.
    Model,
    /// The chunks, levels of detail, materials and bones of a model file.
    ModelInfo,
    /// The Records that refer to the Record selected in the Record tree.
    Incoming,
}

/// Where the preview of an Entry stands.
#[derive(Debug, Clone, Copy)]
pub enum PreviewStatus<'a> {
    /// The Entry is being read.
    Loading,
    /// The Entry is larger than [`PREVIEW_SIZE_LIMIT`] and was not read.
    TooLarge,
    /// The Entry could not be read, for the reason given, or reading stopped without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the Entry was not read or its data was dropped.
    ArchiveChanged,
    /// The Entry is encrypted and the Archive has no key.
    KeyRequired,
    /// The Entry is encrypted and the Archive key does not decrypt it.
    KeyRejected,
    /// The Entry was read.
    Ready(&'a PreviewContents),
}

#[derive(Debug)]
enum Unread {
    Failed(Option<String>),
    ArchiveChanged,
    KeyRequired,
    KeyRejected,
}

#[derive(Debug)]
struct Loaded {
    contents: PreviewContents,
    xml: Option<XmlPreview>,
}

type ReadResult = Result<Loaded, Unread>;

#[derive(Debug)]
enum State {
    Loading(Receiver<ReadResult>),
    TooLarge,
    Failed(Option<String>),
    ArchiveChanged,
    KeyRequired,
    KeyRejected,
    Ready(PreviewContents),
}

/// The preview of one Entry, read on a background thread.
#[derive(Debug)]
pub struct Preview {
    entry: u32,
    state: State,
    image: Option<ImagePreview>,
    audio: Option<AudioPreview>,
    model: Option<ModelPreview>,
    soundbank: Option<SoundbankPreview>,
    xml: Option<XmlPreview>,
    xml_arrived: bool,
}

impl Preview {
    /// Starts reading the Entry `id` of `index` on a background thread, calling `wake` from it
    /// once the data is ready; reads on the calling thread when no thread can be started. A
    /// texture's header also starts its [`ImagePreview`] and a WEM its [`AudioPreview`] on a
    /// thread of its own.
    ///
    /// Nothing is read from an Archive that changed on disk, and data read while it changed is
    /// dropped.
    pub(crate) fn start(
        index: &Arc<EntryIndex>,
        id: u32,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Self {
        if index.watch().check().is_err() {
            return Self {
                entry: id,
                state: State::ArchiveChanged,
                image: None,
                audio: None,
                model: None,
                soundbank: None,
                xml: None,
                xml_arrived: false,
            };
        }
        let wake = Arc::new(wake);
        let image = index
            .entry(id)
            .filter(|entry| Texture::is_header_path(entry.path()))
            .map(|_| {
                let wake = Arc::clone(&wake);
                ImagePreview::start(index, id, move || wake())
            });
        let audio = index
            .entry(id)
            .filter(|entry| is_wem_path(entry.path()))
            .map(|_| {
                let wake = Arc::clone(&wake);
                AudioPreview::start(index, id, move || wake())
            });
        let soundbank = index
            .entry(id)
            .filter(|entry| is_bank_path(entry.path()))
            .map(|_| {
                let wake = Arc::clone(&wake);
                SoundbankPreview::start(index, id, move || wake())
            });
        let model = index
            .entry(id)
            .filter(|entry| is_model_path(entry.path()))
            .map(|_| {
                let wake = Arc::clone(&wake);
                ModelPreview::start(index, id, move || wake())
            });
        let mut preview = Self {
            entry: id,
            state: State::Failed(None),
            image,
            audio,
            model,
            soundbank,
            xml: None,
            xml_arrived: false,
        };
        preview.state = match index.entry(id) {
            None => State::Failed(None),
            Some(entry) if entry.uncompressed_size() > PREVIEW_SIZE_LIMIT => State::TooLarge,
            Some(_) => match spawn(index, id, move || wake()) {
                Spawned::Running(result) => State::Loading(result),
                Spawned::Finished(read) => preview.settle(read),
            },
        };
        preview
    }

    /// Returns the XML preview when the Entry is `CryXml`, known once its start was read.
    #[must_use]
    pub const fn xml(&self) -> Option<&XmlPreview> {
        self.xml.as_ref()
    }

    /// Returns whether the Entry turned out to be `CryXml` since the last call.
    pub(crate) const fn take_xml_arrival(&mut self) -> bool {
        std::mem::replace(&mut self.xml_arrived, false)
    }

    /// Returns the audio preview when the Entry is a WEM, or that of the selected WEM when it is
    /// a soundbank.
    #[must_use]
    pub fn audio(&self) -> Option<&AudioPreview> {
        self.audio
            .as_ref()
            .or_else(|| self.soundbank.as_ref().and_then(SoundbankPreview::audio))
    }

    /// Returns the audio preview for controlling its playback, when the Entry is a WEM or a
    /// soundbank with a selected WEM.
    pub fn audio_mut(&mut self) -> Option<&mut AudioPreview> {
        if self.audio.is_some() {
            self.audio.as_mut()
        } else {
            self.soundbank
                .as_mut()
                .and_then(SoundbankPreview::audio_mut)
        }
    }

    /// Returns the soundbank preview when the Entry is a soundbank.
    #[must_use]
    pub const fn soundbank(&self) -> Option<&SoundbankPreview> {
        self.soundbank.as_ref()
    }

    /// Returns the soundbank preview for selecting a WEM, when the Entry is a soundbank.
    pub(crate) const fn soundbank_mut(&mut self) -> Option<&mut SoundbankPreview> {
        self.soundbank.as_mut()
    }

    /// Returns the model preview when the Entry is a model file.
    #[must_use]
    pub const fn model(&self) -> Option<&ModelPreview> {
        self.model.as_ref()
    }

    /// Returns the model preview for moving its camera, when the Entry is a model file.
    pub const fn model_mut(&mut self) -> Option<&mut ModelPreview> {
        self.model.as_mut()
    }

    /// Returns the image preview when the Entry is the header of a texture.
    #[must_use]
    pub const fn image(&self) -> Option<&ImagePreview> {
        self.image.as_ref()
    }

    /// Returns the image preview for choosing its mip, when the Entry is the header of a texture.
    pub const fn image_mut(&mut self) -> Option<&mut ImagePreview> {
        self.image.as_mut()
    }

    /// Returns the id of the previewed Entry.
    #[must_use]
    pub const fn entry(&self) -> u32 {
        self.entry
    }

    /// Returns where the preview stands.
    #[must_use]
    pub fn status(&self) -> PreviewStatus<'_> {
        match &self.state {
            State::Loading(_) => PreviewStatus::Loading,
            State::TooLarge => PreviewStatus::TooLarge,
            State::Failed(reason) => PreviewStatus::Failed(reason.as_deref()),
            State::ArchiveChanged => PreviewStatus::ArchiveChanged,
            State::KeyRequired => PreviewStatus::KeyRequired,
            State::KeyRejected => PreviewStatus::KeyRejected,
            State::Ready(contents) => PreviewStatus::Ready(contents),
        }
    }

    /// Returns whether the Entry is still being read.
    #[must_use]
    pub const fn is_loading(&self) -> bool {
        matches!(self.state, State::Loading(_))
    }

    /// Returns the data read, or `None` while reading or when nothing was read.
    #[must_use]
    pub const fn contents(&self) -> Option<&PreviewContents> {
        match &self.state {
            State::Ready(contents) => Some(contents),
            _ => None,
        }
    }

    /// Takes the data once it was read and the results of the image, audio and XML previews;
    /// returns whether one arrived.
    pub(crate) fn poll(&mut self) -> bool {
        let imaged = self.image.as_mut().is_some_and(ImagePreview::poll);
        let sounded = self.audio.as_mut().is_some_and(AudioPreview::poll)
            | self.soundbank.as_mut().is_some_and(SoundbankPreview::poll);
        let modelled = self.model.as_mut().is_some_and(ModelPreview::poll);
        let read = self.poll_data();
        let converted = self.xml.as_mut().is_some_and(XmlPreview::poll);
        read || imaged || converted || sounded || modelled
    }

    fn poll_data(&mut self) -> bool {
        let State::Loading(result) = &self.state else {
            return false;
        };
        let read = match result.try_recv() {
            Ok(read) => read,
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => Err(Unread::Failed(None)),
        };
        self.state = self.settle(read);
        true
    }

    fn settle(&mut self, read: ReadResult) -> State {
        match read {
            Ok(Loaded { contents, xml }) => {
                self.xml_arrived = xml.is_some();
                self.xml = xml;
                State::Ready(contents)
            }
            Err(Unread::Failed(reason)) => State::Failed(reason),
            Err(Unread::ArchiveChanged) => State::ArchiveChanged,
            Err(Unread::KeyRequired) => State::KeyRequired,
            Err(Unread::KeyRejected) => State::KeyRejected,
        }
    }
}
