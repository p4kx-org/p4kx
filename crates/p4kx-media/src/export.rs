// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_extraction::{Archive, ArchiveWatch, CancelToken, ConflictPolicy, Exclusion, Filter};

use crate::checkpoint::Checkpoint;
use crate::export_run;
use crate::generation::Generation;
use crate::{Index, Limits, MediaError, MediaKind, Provider, ProviderRegistry};

/// Additional audio artifacts written from eligible retained WEM sources.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioOutput {
    /// Retains original bytes and metadata.
    #[default]
    Raw,
    /// Writes 16-bit PCM WAV from PCM, ADPCM or supported Opus WEM data.
    Wav,
    /// Writes Ogg Vorbis or Ogg Opus from supported original packets.
    Ogg,
}

/// Settings of one media generation.
#[derive(Debug, Clone, Default)]
pub struct ExportOptions {
    /// Repeated kind selection; empty selects every registered provider.
    pub kinds: Vec<MediaKind>,
    /// Include Filter over leaf or ordered container-qualified paths.
    pub include: Option<Filter>,
    /// Exclusion Filters, including split texture header semantics.
    pub exclude: Exclusion,
    /// Conflict policy for the complete root index.
    pub conflict: ConflictPolicy,
    /// Validated streaming bounds.
    pub limits: Limits,
    /// Shared cancellation flag.
    pub cancel: CancelToken,
    /// Retains diagnosed incomplete generations without replacing a complete root index.
    pub diagnostic: bool,
    /// Additional audio artifact format.
    pub audio_output: AudioOutput,
}

/// Observable result of a generation attempt.
#[derive(Debug)]
#[non_exhaustive]
pub enum ExportOutcome {
    /// The complete generation is the current root index.
    Published {
        /// Published index.
        index: Box<Index>,
        /// Whether the root index was replaced.
        changed: bool,
    },
    /// Skip policy retained an existing validated complete generation.
    Skipped {
        /// Existing index that remains current.
        index: Box<Index>,
    },
    /// Diagnosed projection loss was retained separately from the complete root index.
    Incomplete {
        /// Non-complete diagnostic index.
        index: Box<Index>,
        /// Diagnostic index location.
        diagnostic: PathBuf,
    },
}

/// Exports through the permanent production provider registry.
///
/// # Errors
/// Returns capability, safety, source, budget, cancellation or publication failures.
pub fn export(
    archive: &Archive,
    output: &Path,
    options: &ExportOptions,
) -> Result<ExportOutcome, MediaError> {
    export_with(archive, output, options, &ProviderRegistry::default())
}

/// Exports through a caller-supplied ordered provider registry.
///
/// # Errors
/// Returns capability, safety, source, budget, cancellation or publication failures.
pub fn export_with(
    archive: &Archive,
    output: &Path,
    options: &ExportOptions,
    registry: &ProviderRegistry,
) -> Result<ExportOutcome, MediaError> {
    options.limits.validate()?;
    let providers = registry.selected(&options.kinds)?;
    if !options.diagnostic
        && archive
            .issues()
            .iter()
            .any(|(_, issue)| issue.is_archive_wide())
    {
        return Err(MediaError::IncompleteInventory {
            archive: archive.path().to_path_buf(),
        });
    }
    let protection =
        p4kx_extraction::ArchiveWriteProtection::new(archive.path(), options.cancel.flag())?;
    let watch = ArchiveWatch::new(archive);
    let checkpoint = Checkpoint {
        cancel: &options.cancel,
        watch: &watch,
        protection: &protection,
    };
    let mut generation = Generation::new(output, options.limits, &checkpoint)?;
    let result = run_generation(archive, options, &providers, checkpoint, &mut generation);
    generation.finish(result)
}

fn run_generation(
    archive: &Archive,
    options: &ExportOptions,
    providers: &[&dyn Provider],
    checkpoint: Checkpoint<'_>,
    generation: &mut Generation<'_>,
) -> Result<ExportOutcome, MediaError> {
    if options.conflict == ConflictPolicy::Skip
        && let Some(index) = generation.previous.take()
    {
        return Ok(ExportOutcome::Skipped {
            index: Box::new(index),
        });
    }
    let mut index = export_run::build(
        archive,
        generation.stage.permit(),
        options,
        providers,
        checkpoint,
    )?;
    let changed = generation.publish(&mut index, options.conflict, &checkpoint)?;
    let diagnostic = generation
        .output
        .join("diagnostics")
        .join(&index.generation)
        .join("media-index.json");
    if index.complete {
        Ok(ExportOutcome::Published {
            index: Box::new(index),
            changed,
        })
    } else {
        Ok(ExportOutcome::Incomplete {
            index: Box::new(index),
            diagnostic,
        })
    }
}
