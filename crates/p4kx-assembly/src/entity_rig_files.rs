// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry as CacheEntry;
use std::sync::Arc;

use p4kx_chunks::ChunkFile;
use p4kx_extraction::{Entry, EntryOrigin, SourceEncryption, SourceLocation, archive_error_in};
use p4kx_model::SkeletonSource;

use crate::{ArchiveModels, ModelFailure};

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

pub(super) fn read<T, E>(
    models: &ArchiveModels<'_>,
    path: &str,
    cache: &mut InputCache<T>,
    parse: impl FnOnce(&[u8]) -> Result<T, String>,
    check: &mut impl FnMut() -> Result<(), E>,
) -> Result<RigFile<T>, E> {
    check()?;
    let read = match models.rig_entry(path, check)? {
        RigEntry::Missing => Arc::new(RigFileRead::Missing),
        RigEntry::Ambiguous => Arc::new(RigFileRead::Ambiguous),
        RigEntry::Unreadable(reason) => Arc::new(RigFileRead::Unreadable { reason }),
        RigEntry::Unique(entry) => match cache.entry(entry.index()) {
            CacheEntry::Occupied(cached) => Arc::clone(cached.get()),
            CacheEntry::Vacant(slot) => Arc::clone(slot.insert(read_rig_entry(
                models,
                entry,
                |bytes, _| Ok(parse(bytes)),
                check,
            )?)),
        },
    };
    Ok(RigFile {
        path: path.to_owned(),
        read,
    })
}

pub(crate) fn read_rig_entry<T, P, E, F>(
    models: &ArchiveModels<'_>,
    entry: &Entry,
    parse: impl FnOnce(&[u8], &mut F) -> Result<Result<T, P>, E>,
    check: &mut F,
) -> Result<Arc<RigFileRead<T, P>>, E>
where
    F: FnMut() -> Result<(), E>,
{
    let read = match models.read_file_entry_with(entry, u64::MAX, check)? {
        Ok(bytes) => RigFileRead::Read {
            source: models.archive.entry_origin(entry).map(Box::new),
            value: parse(&bytes, check)?,
        },
        Err(error) => read_failure(&error),
    };
    check()?;
    Ok(Arc::new(read))
}

fn read_failure<T, E>(error: &ModelFailure) -> RigFileRead<T, E> {
    match archive_error_in(error) {
        Some(error) if error.is_key_required() => RigFileRead::KeyRequired,
        Some(error) if error.is_wrong_key() => RigFileRead::WrongKey,
        _ => RigFileRead::Unreadable {
            reason: error.to_string(),
        },
    }
}

pub(super) fn definition(bytes: &[u8]) -> Result<RigDefinition, String> {
    let (skeleton, selector) = crate::cdf::read_skeleton_with_position(bytes)?;
    Ok(RigDefinition { skeleton, selector })
}

pub(super) fn skeleton(bytes: &[u8]) -> Result<Option<SkeletonSource>, String> {
    let file = ChunkFile::parse(bytes).map_err(|error| error.to_string())?;
    SkeletonSource::read("animation skeleton", &file).map_err(|error| error.to_string())
}
