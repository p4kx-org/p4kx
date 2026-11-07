// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use clap::{Args, Subcommand};
use p4kx_media::{Limits, MediaKind};

use crate::cli_conflict::Conflict;

/// Arguments of the versioned media export command.
#[derive(Debug, Args)]
pub(crate) struct MediaArgs {
    /// Media operation.
    #[command(subcommand)]
    pub(crate) action: MediaAction,
}

/// Supported media operations.
#[derive(Debug, Subcommand)]
pub(crate) enum MediaAction {
    /// Exports all registered media kinds into one atomic generation.
    Export(Box<MediaExportArgs>),
    /// Prints a versioned media JSON Schema.
    Schema {
        /// Prints the metadata JSONL record schema instead of the index schema.
        #[arg(long)]
        record: bool,
        /// Prints the common typed reference schema.
        #[arg(long, conflicts_with = "record")]
        reference: bool,
        /// Prints the retained source inventory line schema.
        #[arg(long, conflicts_with_all = ["record", "reference"])]
        source: bool,
    },
}

/// Selection, publication and bounds of one media export.
#[derive(Debug, Args)]
pub(crate) struct MediaExportArgs {
    /// Root Archive file.
    pub(crate) archive: PathBuf,
    /// Generation destination directory, created if absent.
    #[arg(long, value_name = "DIR")]
    pub(crate) out: PathBuf,
    /// Requested kind; repeatable. Omission selects every registered provider. Known names:
    /// archive, model, material, texture, shader, animation, animation-definition, audio,
    /// audio-control. A known name without a provider is a non-success capability refusal.
    #[arg(long = "kind", value_name = "KIND")]
    pub(crate) kinds: Vec<MediaKind>,
    /// Includes matching leaf or ordered container-qualified paths, using the search Filter.
    #[arg(long, value_name = "FILTER")]
    pub(crate) filter: Option<String>,
    /// Excludes matching paths and descendants of matching containers; repeatable.
    #[arg(long, value_name = "FILTER")]
    pub(crate) exclude: Vec<String>,
    /// Keeps, replaces, or compares the complete content-addressed generation index.
    #[arg(long, value_enum, default_value_t = Conflict::Skip)]
    pub(crate) conflict: Conflict,
    /// Retains incomplete projections as diagnostic generations; they never replace a complete index.
    #[arg(long)]
    pub(crate) diagnostic: bool,
    /// Resource limits applied before the next unit of work.
    #[command(flatten)]
    pub(crate) limits: MediaLimitsArgs,
}

/// Explicit bounded streaming limits.
#[derive(Debug, Args)]
pub(crate) struct MediaLimitsArgs {
    /// Maximum root Archive bytes to hash.
    #[arg(long = "max-root-bytes", default_value_t = Limits::default().root_bytes, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) root_bytes: u64,
    /// Maximum physical Entries examined, including filtered Entries.
    #[arg(long = "max-sources", default_value_t = Limits::default().sources, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) sources: u64,
    /// Maximum decoded source work, including container traversal.
    #[arg(long = "max-source-bytes", default_value_t = Limits::default().source_bytes, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) source_bytes: u64,
    /// Maximum decoded bytes of one selected Entry or opened container.
    #[arg(long = "max-entry-bytes", default_value_t = Limits::default().entry_bytes, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) entry_bytes: u64,
    /// Maximum metadata and subresource records across all selected providers.
    #[arg(long = "max-records", default_value_t = Limits::default().records, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) records: u64,
    /// Maximum serialised bytes of one metadata record or the index.
    #[arg(long = "max-record-bytes", default_value_t = Limits::default().record_bytes, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) record_bytes: u64,
    /// Maximum bytes of one typed binary chunk.
    #[arg(long = "max-chunk-bytes", default_value_t = Limits::default().chunk_bytes, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) chunk_bytes: u64,
    /// Maximum generation artifact bytes written, counting repeated retention work; index uses the record bound.
    #[arg(long = "max-output-bytes", default_value_t = Limits::default().output_bytes, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) output_bytes: u64,
    /// Maximum ordered container depth; the reader's hard maximum is eight.
    #[arg(long = "max-container-depth", default_value_t = Limits::default().depth, value_parser = clap::value_parser!(u32).range(0..=8))]
    pub(crate) container_depth: u32,
    /// Maximum JSON value depth, including the envelope; at most 512.
    #[arg(long = "max-value-depth", default_value_t = Limits::default().value_depth, value_parser = clap::value_parser!(u32).range(1..=512))]
    pub(crate) value_depth: u32,
    /// Maximum reference queries across the generation.
    #[arg(long = "max-references", default_value_t = Limits::default().references, value_parser = clap::value_parser!(u64).range(1..))]
    pub(crate) references: u64,
    /// Maximum candidates retained for one reference resolution.
    #[arg(long = "max-reference-candidates", default_value_t = Limits::default().reference_candidates, value_parser = clap::value_parser!(u32).range(1..))]
    pub(crate) reference_candidates: u32,
}

impl MediaLimitsArgs {
    pub(crate) fn limits(&self) -> Limits {
        let mut limits = Limits::default();
        limits.root_bytes = self.root_bytes;
        limits.sources = self.sources;
        limits.source_bytes = self.source_bytes;
        limits.entry_bytes = self.entry_bytes;
        limits.records = self.records;
        limits.record_bytes = self.record_bytes;
        limits.chunk_bytes = self.chunk_bytes;
        limits.output_bytes = self.output_bytes;
        limits.depth = self.container_depth;
        limits.value_depth = self.value_depth;
        limits.references = self.references;
        limits.reference_candidates = self.reference_candidates;
        limits
    }
}
