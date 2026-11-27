// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod worker;

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::thread;

use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};

use super::{ImageHeaderProvenance, ImageMipProvenance, TextureInfo};
use crate::EntryIndex;

static SERIAL: AtomicU64 = AtomicU64::new(0);

/// Where the image preview of a texture stands.
#[derive(Debug, Clone, Copy)]
pub enum ImageStatus<'a> {
    /// The texture's header is being read.
    Opening,
    /// The texture could not be opened, for the reason given, or opening stopped without a
    /// result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the texture was not read or its data was dropped.
    ArchiveChanged,
    /// The header was read; the mip stands as given. A format the preview cannot decode has no
    /// mips to show.
    Opened(&'a TextureInfo, MipStatus<'a>),
}

/// Where the shown mip of an opened texture stands.
#[derive(Debug, Clone, Copy)]
pub enum MipStatus<'a> {
    /// No mip was chosen yet.
    Unchosen,
    /// The mip of this level is being decoded.
    Decoding(u32),
    /// The mip of this level could not be decoded, for the reason given, or decoding stopped
    /// without a result.
    Failed(u32, Option<&'a str>),
    /// The mip of this level was decoded; its texture once the window took it over.
    Decoded(u32, Option<TextureId>),
}

#[derive(Debug)]
enum Event {
    Opened(Box<OpenedImage>),
    Failed(Option<String>),
    ArchiveChanged,
    Decoded(u32, Result<DecodedImage, String>),
}

#[derive(Debug)]
struct OpenedImage {
    info: TextureInfo,
    provenance: ImageHeaderProvenance,
}

#[derive(Debug)]
struct DecodedImage {
    pixels: ColorImage,
    provenance: ImageMipProvenance,
}

enum Mip {
    Unchosen,
    Decoding(u32),
    Failed(u32, Option<String>),
    Decoded {
        level: u32,
        image: Option<ColorImage>,
        texture: Option<TextureHandle>,
        provenance: ImageMipProvenance,
    },
}

impl fmt::Debug for Mip {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unchosen => f.write_str("Unchosen"),
            Self::Decoding(level) => f.debug_tuple("Decoding").field(level).finish(),
            Self::Failed(level, reason) => {
                f.debug_tuple("Failed").field(level).field(reason).finish()
            }
            Self::Decoded { level, texture, .. } => f
                .debug_struct("Decoded")
                .field("level", level)
                .field("uploaded", &texture.is_some())
                .finish_non_exhaustive(),
        }
    }
}

#[derive(Debug)]
enum State {
    Opening,
    Failed(Option<String>),
    ArchiveChanged,
    Opened(Box<OpenedImage>, Mip),
}

/// The image preview of a texture: its header read and its mips decoded on a background
/// thread, one requested level at a time.
pub struct ImagePreview {
    serial: u64,
    dropped: Arc<AtomicBool>,
    index: Arc<EntryIndex>,
    requests: Option<Sender<u32>>,
    events: Option<Receiver<Event>>,
    state: State,
}

impl ImagePreview {
    /// Starts opening the texture whose header is the Entry `id` of `index` on a background
    /// thread, calling `wake` from it whenever a result is ready.
    pub(crate) fn start(
        index: &Arc<EntryIndex>,
        id: u32,
        wake: impl Fn() + Send + 'static,
    ) -> Self {
        let (requests, requested) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let dropped = Arc::new(AtomicBool::new(false));
        let thread_index = Arc::clone(index);
        let thread_dropped = Arc::clone(&dropped);
        let spawned = thread::Builder::new()
            .name("image-preview".to_owned())
            .spawn(move || {
                worker::run(
                    &thread_index,
                    id,
                    &requested,
                    &sender,
                    &thread_dropped,
                    &wake,
                );
            });
        let state = match spawned {
            Ok(_handle) => State::Opening,
            Err(_ignored) => State::Failed(None),
        };
        Self {
            serial: SERIAL.fetch_add(1, Ordering::Relaxed),
            dropped,
            index: Arc::clone(index),
            requests: Some(requests),
            events: Some(events),
            state,
        }
    }

    /// Returns a number no other image preview of this process has.
    #[must_use]
    pub const fn serial(&self) -> u64 {
        self.serial
    }

    /// Returns where the preview stands.
    #[must_use]
    pub fn status(&self) -> ImageStatus<'_> {
        match self.current_state() {
            None | Some(State::ArchiveChanged) => ImageStatus::ArchiveChanged,
            Some(State::Opening) => ImageStatus::Opening,
            Some(State::Failed(reason)) => ImageStatus::Failed(reason.as_deref()),
            Some(State::Opened(opened, mip)) => ImageStatus::Opened(&opened.info, mip_status(mip)),
        }
    }

    /// Returns provenance for the header of the currently opened texture.
    #[must_use]
    pub fn header_provenance(&self) -> Option<&ImageHeaderProvenance> {
        let State::Opened(opened, _) = self.current_state()? else {
            return None;
        };
        Some(&opened.provenance)
    }

    /// Returns provenance for the accepted decoded mip, including after its pixels are uploaded.
    #[must_use]
    pub fn mip_provenance(&self) -> Option<&ImageMipProvenance> {
        let State::Opened(_, Mip::Decoded { provenance, .. }) = self.current_state()? else {
            return None;
        };
        Some(provenance)
    }

    fn current_state(&self) -> Option<&State> {
        (!self.index.watch().is_changed()).then_some(&self.state)
    }

    /// Returns whether the header is being read or a mip is being decoded.
    #[must_use]
    pub const fn is_busy(&self) -> bool {
        matches!(
            self.state,
            State::Opening | State::Opened(_, Mip::Decoding(_))
        )
    }

    /// Returns the level shown or being decoded, or `None` when none was chosen.
    #[must_use]
    pub const fn level(&self) -> Option<u32> {
        match &self.state {
            State::Opened(
                _,
                Mip::Decoding(level) | Mip::Failed(level, _) | Mip::Decoded { level, .. },
            ) => Some(*level),
            _ => None,
        }
    }

    /// Decodes mip `level` on the background thread unless it is shown or being decoded
    /// already; does nothing before the header was read, for a format the preview cannot
    /// decode, and for a level that is not showable within `max_side` pixels a side.
    pub fn request(&mut self, level: u32, max_side: u32) {
        if self.index.watch().is_changed() {
            self.discard_changed();
            return;
        }
        let (Some(requests), State::Opened(opened, mip)) = (&self.requests, &mut self.state) else {
            return;
        };
        let info = &opened.info;
        let current = matches!(
            mip,
            Mip::Decoding(shown) | Mip::Decoded { level: shown, .. } if *shown == level
        );
        if current || !info.is_showable(level, max_side) || !info.is_decodable() {
            return;
        }
        *mip = match requests.send(level) {
            Ok(()) => Mip::Decoding(level),
            Err(_closed) => Mip::Failed(level, None),
        };
    }

    /// Hands a decoded mip to `ctx` as a texture once, giving up the decoded pixels.
    pub(crate) fn upload(&mut self, ctx: &Context) {
        if self.index.watch().is_changed() {
            self.discard_changed();
            return;
        }
        if let State::Opened(
            _,
            Mip::Decoded {
                level,
                image,
                texture,
                ..
            },
        ) = &mut self.state
            && let Some(image) = image.take()
        {
            *texture =
                Some(ctx.load_texture(format!("p4kx-mip-{level}"), image, TextureOptions::NEAREST));
        }
    }

    /// Takes the results the background thread sent; returns whether one arrived.
    pub(crate) fn poll(&mut self) -> bool {
        if self.index.watch().is_changed() {
            return self.discard_changed();
        }
        let mut changed = false;
        while let Some(events) = &self.events {
            match events.try_recv() {
                Ok(event) => {
                    self.apply(event);
                    changed = true;
                }
                Err(TryRecvError::Empty) => return changed,
                Err(TryRecvError::Disconnected) => return self.lost() || changed,
            }
        }
        changed
    }

    fn discard_changed(&mut self) -> bool {
        if matches!(self.state, State::ArchiveChanged) {
            return false;
        }
        self.dropped.store(true, Ordering::Release);
        self.requests.take();
        self.events.take();
        self.state = State::ArchiveChanged;
        true
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Opened(info) => self.state = State::Opened(info, Mip::Unchosen),
            Event::Failed(reason) => self.state = State::Failed(reason),
            Event::ArchiveChanged => {
                self.discard_changed();
            }
            Event::Decoded(level, decoded) => {
                if let State::Opened(_, mip) = &mut self.state
                    && matches!(mip, Mip::Decoding(pending) if *pending == level)
                {
                    *mip = match decoded {
                        Ok(decoded) => Mip::Decoded {
                            level,
                            image: Some(decoded.pixels),
                            texture: None,
                            provenance: decoded.provenance,
                        },
                        Err(reason) => Mip::Failed(level, Some(reason)),
                    };
                }
            }
        }
    }

    fn lost(&mut self) -> bool {
        match &mut self.state {
            State::Opening => self.state = State::Failed(None),
            State::Opened(_, mip @ Mip::Decoding(_)) => {
                if let Mip::Decoding(level) = *mip {
                    *mip = Mip::Failed(level, None);
                }
            }
            _ => return false,
        }
        true
    }
}

impl fmt::Debug for ImagePreview {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ImagePreview")
            .field("serial", &self.serial)
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl Drop for ImagePreview {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

fn mip_status(mip: &Mip) -> MipStatus<'_> {
    match mip {
        Mip::Unchosen => MipStatus::Unchosen,
        Mip::Decoding(level) => MipStatus::Decoding(*level),
        Mip::Failed(level, reason) => MipStatus::Failed(*level, reason.as_deref()),
        Mip::Decoded { level, texture, .. } => {
            MipStatus::Decoded(*level, texture.as_ref().map(TextureHandle::id))
        }
    }
}
