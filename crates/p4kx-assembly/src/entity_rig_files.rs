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

/// Definition, skeleton and parameters selected by one model path.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ModelRig {
    /// Authored character definition when the model path selects one.
    pub definition: Option<RigFile<RigDefinition>>,
    /// Skeleton selected by the model or its definition.
    pub skeleton: Option<RigFile<Option<SkeletonSource>>>,
    /// Parameter file associated with the selected skeleton.
    pub parameters: Option<RigFile<Vec<super::RigParameter>>>,
}

#[derive(Default)]
pub(super) struct RigFiles {
    definitions: InputCache<RigDefinition>,
    skeletons: InputCache<Option<SkeletonSource>>,
    parameters: InputCache<Vec<super::RigParameter>>,
}

#[derive(Clone, Copy)]
pub(super) enum RigPath<'a> {
    Authored(&'a str),
    Exact(&'a str),
}

impl<'path> RigPath<'path> {
    const fn as_str(self) -> &'path str {
        match self {
            Self::Authored(path) | Self::Exact(path) => path,
        }
    }

    const fn companion(self, path: &str) -> RigPath<'_> {
        match self {
            Self::Authored(_) => RigPath::Authored(path),
            Self::Exact(_) => RigPath::Exact(path),
        }
    }
}

impl RigFiles {
    pub(super) fn model<I: RigInputs>(
        &mut self,
        models: &ArchiveModels<'_>,
        path: RigPath<'_>,
        inputs: &mut I,
    ) -> Result<ModelRig, I::Error> {
        let definition = crate::cdf::is_character_definition(path.as_str())
            .then(|| read(models, path, &mut self.definitions, definition, inputs))
            .transpose()?;
        let skeleton_path = definition.as_ref().map_or(Some(path), |file| {
            file.value()
                .and_then(|value| value.skeleton.as_deref())
                .map(RigPath::Authored)
        });
        let skeleton = skeleton_path
            .map(|path| read(models, path, &mut self.skeletons, skeleton, inputs))
            .transpose()?;
        let parameters = skeleton_path
            .map(|path| {
                let companion = crate::gear_clip::params_path(path.as_str());
                read(
                    models,
                    path.companion(&companion),
                    &mut self.parameters,
                    crate::gear_params::catalogue_parameters,
                    inputs,
                )
            })
            .transpose()?;
        Ok(ModelRig {
            definition,
            skeleton,
            parameters,
        })
    }
}

fn read<T, I: RigInputs>(
    models: &ArchiveModels<'_>,
    path: RigPath<'_>,
    cache: &mut InputCache<T>,
    parse: impl FnOnce(&[u8]) -> Result<T, String>,
    inputs: &mut I,
) -> Result<RigFile<T>, I::Error> {
    inputs.check()?;
    let mut check = || inputs.check();
    let entry = match path {
        RigPath::Authored(path) => models.rig_entry(path, &mut check)?,
        RigPath::Exact(path) => models.exact_rig_entry(path, &mut check)?,
    };
    let read = match entry {
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
        path: path.as_str().to_owned(),
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
