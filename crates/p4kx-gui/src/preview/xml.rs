// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

use p4kx_extraction::ObservedEntry;
use p4kx_extraction::cryxml::{self, Format};

use super::xml_document::{CappedWriter, XML_TEXT_LIMIT, XmlDocument};
use super::xml_preview_origin::PositionedXml;
use crate::EntryIndex;
use crate::model::describe;

/// The largest `CryXml` Entry, in bytes, the XML tab converts.
pub const XML_SIZE_LIMIT: u64 = 64 * 1024 * 1024;

const READ_CHUNK: u64 = 1024 * 1024;

#[cfg(test)]
#[path = "xml_origin_tests.rs"]
mod origin_tests;

/// Returns whether data starting with `start` is `CryXml`; text XML and other data are not.
pub(super) fn is_cryxml(start: &[u8]) -> bool {
    matches!(cryxml::detect(start), Some(Format::CryXml(_)))
}

/// Where the XML preview of a `CryXml` Entry stands.
#[derive(Debug, Clone, Copy)]
pub enum XmlStatus<'a> {
    /// The Entry is being read and converted.
    Converting,
    /// The Entry is larger than [`XML_SIZE_LIMIT`] and was not converted.
    TooLarge,
    /// The Entry could not be read or converted, for the reason given, or converting stopped
    /// without a result.
    Failed(Option<&'a str>),
    /// The Archive changed on disk, so the converted XML was dropped.
    ArchiveChanged,
    /// The Entry was converted.
    Ready(&'a XmlDocument),
}

#[derive(Debug)]
enum Converted {
    TooLarge,
    Failed(String),
    ArchiveChanged,
    Ready(XmlDocument),
}

#[derive(Debug)]
enum State {
    Converting(Receiver<Converted>),
    Settled(Converted),
    Lost,
}

/// The XML preview of a `CryXml` Entry, converted on a background thread.
#[derive(Debug)]
pub struct XmlPreview {
    dropped: Arc<AtomicBool>,
    state: State,
}

/// The conversion an [`XmlPreview`] waits for, run by the thread that read the Entry's start.
pub(super) struct XmlJob {
    dropped: Arc<AtomicBool>,
    sender: Sender<Converted>,
}

impl XmlPreview {
    /// Creates a preview waiting for the conversion of its [`XmlJob`].
    pub(super) fn pending() -> (Self, XmlJob) {
        let (sender, receiver) = mpsc::channel();
        let dropped = Arc::new(AtomicBool::new(false));
        let job = XmlJob {
            dropped: Arc::clone(&dropped),
            sender,
        };
        let preview = Self {
            dropped,
            state: State::Converting(receiver),
        };
        (preview, job)
    }

    /// Returns where the preview stands.
    #[must_use]
    pub fn status(&self) -> XmlStatus<'_> {
        match &self.state {
            State::Converting(_) => XmlStatus::Converting,
            State::Settled(Converted::TooLarge) => XmlStatus::TooLarge,
            State::Settled(Converted::Failed(reason)) => XmlStatus::Failed(Some(reason)),
            State::Settled(Converted::ArchiveChanged) => XmlStatus::ArchiveChanged,
            State::Settled(Converted::Ready(text)) => XmlStatus::Ready(text),
            State::Lost => XmlStatus::Failed(None),
        }
    }

    /// Returns the converted XML, or `None` while converting or when there is none.
    #[must_use]
    pub const fn text(&self) -> Option<&XmlDocument> {
        match &self.state {
            State::Settled(Converted::Ready(text)) => Some(text),
            _ => None,
        }
    }

    /// Takes the result of the conversion; returns whether it arrived.
    pub(super) fn poll(&mut self) -> bool {
        let State::Converting(receiver) = &self.state else {
            return false;
        };
        self.state = match receiver.try_recv() {
            Ok(converted) => State::Settled(converted),
            Err(TryRecvError::Empty) => return false,
            Err(TryRecvError::Disconnected) => State::Lost,
        };
        true
    }
}

impl Drop for XmlPreview {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

impl XmlJob {
    /// Reads the rest of an Entry of `size` bytes from `reader` after `start`, converts it and
    /// sends the result; calls `wake` once it was sent. Stops early once the preview is dropped.
    pub(super) fn run(
        self,
        index: &EntryIndex,
        input: (&[u8], u64, Option<ObservedEntry<'_>>),
        reader: &mut impl Read,
        wake: &dyn Fn(),
    ) {
        let Some(converted) = self.convert(index, input, reader) else {
            return;
        };
        if !self.dropped.load(Ordering::Acquire) && self.sender.send(converted).is_ok() {
            wake();
        }
    }

    fn convert(
        &self,
        index: &EntryIndex,
        input: (&[u8], u64, Option<ObservedEntry<'_>>),
        reader: &mut impl Read,
    ) -> Option<Converted> {
        let (start, size, original) = input;
        if size > XML_SIZE_LIMIT {
            return Some(Converted::TooLarge);
        }
        let Ok(capacity) = usize::try_from(size) else {
            return Some(Converted::TooLarge);
        };
        let mut data = Vec::with_capacity(capacity.max(start.len()));
        data.extend_from_slice(start);
        let mut remaining = size.saturating_sub(start.len() as u64);
        while remaining > 0 {
            if self.is_dropped() {
                return None;
            }
            let read = reader
                .by_ref()
                .take(remaining.min(READ_CHUNK))
                .read_to_end(&mut data);
            match read {
                Ok(0) => break,
                Ok(count) => remaining = remaining.saturating_sub(count as u64),
                Err(_) if index.watch().check().is_err() => {
                    return Some(Converted::ArchiveChanged);
                }
                Err(error) => return Some(Converted::Failed(describe(&error))),
            }
        }
        if index.watch().check().is_err() {
            return Some(Converted::ArchiveChanged);
        }
        let rendered = self.render(data, original, XML_TEXT_LIMIT);
        if index.watch().check().is_err() {
            Some(Converted::ArchiveChanged)
        } else {
            rendered
        }
    }

    fn render(
        &self,
        data: Vec<u8>,
        original: Option<ObservedEntry<'_>>,
        limit: usize,
    ) -> Option<Converted> {
        let document = match cryxml::parse(&data) {
            Ok(document) => document,
            Err(error) => return Some(Converted::Failed(describe(&error))),
        };
        let mut writer = CappedWriter::new(limit, &self.dropped);
        let mut positioned = PositionedXml::new();
        if let Err(error) = document.write_xml_positioned(&mut writer, |span| {
            positioned.observe(span);
            Ok(())
        }) && !writer.capped
        {
            return (!self.is_dropped()).then(|| Converted::Failed(describe(&error)));
        }
        drop(document);
        drop(data);
        if self.is_dropped() {
            return None;
        }
        let document = XmlDocument::new(writer.out, writer.capped);
        let origin = positioned.finish(original, document.bytes().len() as u64, &self.dropped);
        (!self.is_dropped()).then(|| Converted::Ready(document.with_provenance(origin)))
    }

    fn is_dropped(&self) -> bool {
        self.dropped.load(Ordering::Acquire)
    }
}
