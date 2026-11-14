// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry as CacheEntry;
use std::sync::Arc;

use p4kx_chunks::ChunkFile;
use p4kx_extraction::{Entry, EntryOrigin, SourceEncryption, SourceLocation, archive_error_in};
use p4kx_model::SkeletonSource;

use crate::ArchiveModels;

/// An authored file request and its actual discovery outcome.
#[derive(Debug)]
#[non_exhaustive]
pub struct RigFile<T, E = String> {
    /// Requested path, independent of archive lookup normalization.
    pub path: String,
    /// Whether the bytes were read and parsed.
    pub read: Arc<RigFileRead<T, E>>,
}

impl<T, E> RigFile<T, E> {
    /// Returns the parsed value only when a source was successfully read.
    #[must_use]
    pub fn value(&self) -> Option<&T> {
        match self.read.as_ref() {
            RigFileRead::Read {
                value: Ok(value), ..
            } => Some(value),
            _ => None,
        }
    }

    /// Returns the original Archive input whose bytes were successfully read.
    #[must_use]
    pub fn source(&self) -> Option<&EntryOrigin> {
        match self.read.as_ref() {
            RigFileRead::Read { source, .. } => source.as_deref(),
            _ => None,
        }
    }

    /// Returns the payload encryption evidence, unknown for an unsuccessful read.
    #[must_use]
    pub fn encryption(&self) -> SourceEncryption {
        self.source()
            .map_or(SourceEncryption::Unknown, EntryOrigin::encryption)
    }
}

/// Whether rig discovery was able to consume the requested original input.
#[derive(Debug)]
#[non_exhaustive]
pub enum RigFileRead<T, E = String> {
    /// The lookup found no matching Archive Entry.
    Missing,
    /// More than one physical Archive Entry matches the requested path.
    Ambiguous,
    /// This physical source was outside the caller's selected projection.
    NotSelected,
    /// The source demanded a key, so its payload was unavailable.
    KeyRequired,
    /// The provided key failed to recover the payload.
    WrongKey,
    /// A source or lookup failed before a complete input could be read.
    Unreadable {
        /// Original read or lookup diagnostic when available.
        reason: String,
    },
    /// The original bytes were read, regardless of whether the parser could interpret them.
    Read {
        /// Physical Entry when the Archive can establish its identity.
        source: Option<Box<EntryOrigin>>,
        /// Parsed metadata or the original parser diagnostic.
        value: Result<T, E>,
    },
}

/// Skeleton selection declared by a character definition.
#[derive(Debug)]
#[non_exhaustive]
pub struct RigDefinition {
    /// The Model File written by the definition, if present.
    pub skeleton: Option<String>,
    /// Original position of that attribute when available.
    pub selector: Option<SourceLocation>,
}

/// A caller-admitted original rig dependency, before interpreting its bytes.
#[derive(Debug)]
#[non_exhaustive]
pub enum RigInput {
    /// Exact decoded payload from the observed Entry.
    Read(Vec<u8>),
    /// The original Entry was deliberately excluded from byte inspection.
    NotSelected,
    /// The read failed without a usable key.
    KeyRequired,
    /// The supplied key failed to recover the payload.
    WrongKey,
    /// A selected payload could not be read.
    Unreadable(String),
}

/// Checkpoints and reads physical inputs within one chosen Archive scope.
pub trait RigInputs {
    /// The caller's terminal admission, cancellation or source-change failure.
    type Error;

    /// Checks whether work on the current inventory can continue.
    ///
    /// # Errors
    /// Returns the caller's original terminal error.
    fn check(&mut self) -> Result<(), Self::Error>;

    /// Reads the exact physically selected Entry or describes its omission.
    ///
    /// # Errors
    /// Returns only terminal admission, cancellation or source-change failures.
    fn read(&mut self, entry: &Entry) -> Result<RigInput, Self::Error>;
}

pub(crate) struct ArchiveRigInputs<'models, 'archive, F> {
    pub(crate) models: &'models ArchiveModels<'archive>,
    pub(crate) check: F,
}

impl<E, F: FnMut() -> Result<(), E>> RigInputs for ArchiveRigInputs<'_, '_, F> {
    type Error = E;

    fn check(&mut self) -> Result<(), E> {
        (self.check)()
    }

    fn read(&mut self, entry: &Entry) -> Result<RigInput, E> {
        let result = self
            .models
            .read_file_entry_with(entry, u64::MAX, &mut self.check)?;
        Ok(match result {
            Ok(bytes) => RigInput::Read(bytes),
            Err(error) => match archive_error_in(&error) {
                Some(error) if error.is_key_required() => RigInput::KeyRequired,
                Some(error) if error.is_wrong_key() => RigInput::WrongKey,
                _ => RigInput::Unreadable(error.to_string()),
            },
        })
    }
}

pub(crate) enum RigEntry<'archive> {
    Missing,
    Ambiguous,
    Unreadable(String),
    Unique(&'archive Entry),
}

type InputCache<T> = HashMap<usize, Arc<RigFileRead<T>>>;

#[derive(Default)]
pub(super) struct RigFiles {
    pub(super) definitions: InputCache<RigDefinition>,
    pub(super) skeletons: InputCache<Option<SkeletonSource>>,
    pub(super) parameters: InputCache<Vec<super::RigParameter>>,
}

pub(super) fn read<T, I: RigInputs>(
    models: &ArchiveModels<'_>,
    path: &str,
    cache: &mut InputCache<T>,
    parse: impl FnOnce(&[u8]) -> Result<T, String>,
    inputs: &mut I,
) -> Result<RigFile<T>, I::Error> {
    inputs.check()?;
    let read = match models.rig_entry(path, &mut || inputs.check())? {
        RigEntry::Missing => Arc::new(RigFileRead::Missing),
        RigEntry::Ambiguous => Arc::new(RigFileRead::Ambiguous),
        RigEntry::Unreadable(reason) => Arc::new(RigFileRead::Unreadable { reason }),
        RigEntry::Unique(entry) => match cache.entry(entry.index()) {
            CacheEntry::Occupied(cached) => Arc::clone(cached.get()),
            CacheEntry::Vacant(slot) => Arc::clone(slot.insert(read_rig_entry(
                models,
                entry,
                |bytes, _| Ok(parse(bytes)),
                inputs,
            )?)),
        },
    };
    Ok(RigFile {
        path: path.to_owned(),
        read,
    })
}

pub(crate) fn read_rig_entry<T, P, I: RigInputs>(
    models: &ArchiveModels<'_>,
    entry: &Entry,
    parse: impl FnOnce(&[u8], &mut I) -> Result<Result<T, P>, I::Error>,
    inputs: &mut I,
) -> Result<Arc<RigFileRead<T, P>>, I::Error> {
    let read = match inputs.read(entry)? {
        RigInput::Read(bytes) => RigFileRead::Read {
            source: models.archive.entry_origin(entry).map(Box::new),
            value: parse(&bytes, inputs)?,
        },
        RigInput::NotSelected => RigFileRead::NotSelected,
        RigInput::WrongKey => RigFileRead::WrongKey,
        RigInput::KeyRequired => RigFileRead::KeyRequired,
        RigInput::Unreadable(reason) => RigFileRead::Unreadable { reason },
    };
    inputs.check()?;
    Ok(Arc::new(read))
}

pub(super) fn definition(bytes: &[u8]) -> Result<RigDefinition, String> {
    let (skeleton, selector) = crate::cdf::read_skeleton_with_position(bytes)?;
    Ok(RigDefinition { skeleton, selector })
}

pub(super) fn skeleton(bytes: &[u8]) -> Result<Option<SkeletonSource>, String> {
    let file = ChunkFile::parse(bytes).map_err(|error| error.to_string())?;
    SkeletonSource::read("animation skeleton", &file).map_err(|error| error.to_string())
}
