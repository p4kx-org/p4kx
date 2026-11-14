// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};

use egui::{Color32, ColorImage};
use p4kx_extraction::texture_preview::ObservedPreviewImage;
use p4kx_extraction::{RgbaImage, Texture};
use rayon::prelude::*;

use super::{DecodedImage, Event, OpenedImage};
use crate::model::describe;
use crate::{EntryIndex, ImageHeaderProvenance, ImageMipProvenance, TextureInfo};

pub(super) fn run(
    index: &EntryIndex,
    id: u32,
    requests: &Receiver<u32>,
    events: &Sender<Event>,
    dropped: &AtomicBool,
    wake: &impl Fn(),
) {
    let worker = Worker {
        index,
        requests,
        events,
        dropped,
        wake,
    };
    worker.run(id);
}

struct Worker<'a, Wake> {
    index: &'a EntryIndex,
    requests: &'a Receiver<u32>,
    events: &'a Sender<Event>,
    dropped: &'a AtomicBool,
    wake: &'a Wake,
}

impl<Wake: Fn()> Worker<'_, Wake> {
    fn send(&self, event: Event) -> bool {
        let sent = !self.is_dropped() && self.events.send(event).is_ok();
        if sent {
            (self.wake)();
        }
        sent
    }

    fn is_dropped(&self) -> bool {
        self.dropped.load(Ordering::Acquire)
    }

    fn current(&self) -> bool {
        if self.index.watch().check().is_ok() {
            return true;
        }
        self.send(Event::ArchiveChanged);
        false
    }

    fn run(&self, id: u32) {
        let Some(texture) = self.open(id) else { return };
        let mut next = self.requests.recv().ok();
        while let Some(requested) = next {
            let level = self.requests.try_iter().last().unwrap_or(requested);
            if self.is_dropped() {
                return;
            }
            let decoded = texture.decode_preview_observed(level);
            if self.is_dropped() {
                return;
            }
            if let Some(newer) = self.requests.try_iter().last() {
                next = Some(newer);
                continue;
            }
            let decoded = decoded
                .map_err(|error| describe(&error))
                .and_then(decoded_image);
            if !self.current() || !self.send(Event::Decoded(level, decoded)) {
                return;
            }
            next = self.requests.recv().ok();
        }
    }

    fn open(&self, id: u32) -> Option<Texture<'_>> {
        let (Some(archive), Some(entry)) = (self.index.archive(), self.index.entry(id)) else {
            self.send(Event::Failed(None));
            return None;
        };
        if !self.current() {
            return None;
        }
        let opened = Texture::from_entry(archive, entry)
            .map_err(|error| describe(&error))
            .and_then(|texture| {
                let info = TextureInfo::of(texture.header());
                let provenance =
                    ImageHeaderProvenance::capture(&texture).map_err(|error| describe(&error))?;
                Ok((texture, Box::new(OpenedImage { info, provenance })))
            });
        if !self.current() {
            return None;
        }
        match opened {
            Ok((texture, opened)) => {
                let decodable = opened.info.is_decodable();
                (self.send(Event::Opened(opened)) && decodable).then_some(texture)
            }
            Err(reason) => {
                self.send(Event::Failed(Some(reason)));
                None
            }
        }
    }
}

fn decoded_image(observed: ObservedPreviewImage<'_>) -> Result<DecodedImage, String> {
    let provenance = ImageMipProvenance::capture(&observed).map_err(|error| describe(&error))?;
    Ok(DecodedImage {
        pixels: colour_image(&observed.into_image()),
        provenance,
    })
}

fn colour_image(image: &RgbaImage) -> ColorImage {
    let pixels = image
        .pixels()
        .par_chunks_exact(4)
        .map(|pixel| match *pixel {
            [r, g, b, a] => Color32::from_rgba_unmultiplied(r, g, b, a),
            _ => Color32::TRANSPARENT,
        })
        .collect();
    ColorImage::new([image.width() as usize, image.height() as usize], pixels)
}
