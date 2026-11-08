// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::Args;

use crate::cli_conflict::Conflict;
use crate::cli_output::{AudioOutput, TextureOutput};

/// Arguments of `p4kx extract`.
#[derive(Debug, Args)]
pub(crate) struct ExtractArgs {
    /// Path of the Data.p4k Archive.
    pub(crate) archive: PathBuf,
    /// Paths of the Entries to extract, with `\` or `/` as separator, ignoring case.
    #[arg(required_unless_present = "filter", conflicts_with = "filter")]
    pub(crate) entries: Vec<String>,
    /// Extracts every Entry the Filter matches instead of the given paths.
    #[arg(long, value_name = "FILTER")]
    pub(crate) filter: Option<String>,
    /// Leaves out every Entry this Filter matches, with the rules of the Filter; repeatable. An
    /// Entry is extracted when --filter matches it and no --exclude does. A texture whose header
    /// it matches is left out with its parts; with --convert, a part it matches is not written
    /// but still read to build its texture, which a note on stderr tells. A pair that leaves
    /// nothing is invalid input.
    #[arg(long, value_name = "FILTER", conflicts_with = "entries")]
    pub(crate) exclude: Vec<String>,
    /// Target folder; created if missing, files already in it are handled by --conflict. Defaults
    /// to a new folder `SC-EXPORT-<Channel>-<Build version>-<timestamp>` in the current directory.
    #[arg(long, value_name = "FOLDER")]
    pub(crate) out: Option<PathBuf>,
    /// What to do with a file that already exists in the target.
    #[arg(long, value_enum, default_value_t = Conflict::Skip)]
    pub(crate) conflict: Conflict,
    /// Converts CryXml Entries to UTF-8 XML, DDS textures as --textures says, WEM sounds and
    /// soundbanks as --audio says and the DataForge Entry to one Record XML file per Record below
    /// its folder instead of the .dcb; text XML and every other Entry are written unchanged.
    #[arg(long)]
    pub(crate) convert: bool,
    /// What --convert writes for a texture `<name>.dds`, its parts `<name>.dds.1` … .N and any
    /// alpha tail `<name>.dds.a`: `merge` one standard DDS under the header's path (and, without
    /// `png`, the tail merged under its own path), `png` the mip --mip as `<name>.png` with the
    /// tail as alpha, a high-range texture as OpenEXR `<name>.exr`
    /// and a cubemap as six face files, `raw`
    /// the Entries unchanged (with `merge`, only the parts); a texture that cannot be converted
    /// is reported and written raw.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        num_args = 1..,
        default_values_t = [TextureOutput::Merge, TextureOutput::Png],
        requires = "convert",
        value_name = "OUTPUTS"
    )]
    pub(crate) textures: Vec<TextureOutput>,
    /// The mip level --convert exports as PNG or OpenEXR, 0 being the largest; a texture with
    /// fewer levels exports its smallest.
    #[arg(long, default_value_t = 0, requires = "convert", value_name = "LEVEL")]
    pub(crate) mip: u32,
    /// What --convert writes for a WEM sound `<name>.wem` and for each WEM a soundbank `<name>.bnk`
    /// embeds; a soundbank is also written unchanged and unpacked into the folder `<name>` next to
    /// it, each WEM named `<ID>`. A sound whose codec cannot be decoded is reported and written
    /// unchanged as .wem, as are plug-in media and the prefetched start of a streamed WEM.
    #[arg(
        long,
        value_enum,
        default_value_t = AudioOutput::Ogg,
        requires = "convert",
        value_name = "FORMAT"
    )]
    pub(crate) audio: AudioOutput,
    /// Checks each Entry against the CRC-32C of the Archive directory while it is written: an
    /// Entry that does not match is not written and counted as a checksum mismatch in the
    /// summary and as `mismatched` in the JSON report, and the exit code is 1. Costs one
    /// checksum pass over the extracted bytes. An Entry skipped because its file exists is not read
    /// and so not verified; an unchanged file under the if-changed Conflict policy matched the
    /// stored CRC-32C.
    #[arg(long)]
    pub(crate) verify: bool,
    /// The most threads the extraction uses, sound conversion included; by default one per
    /// logical processor, and a larger value is capped there. Fewer threads leave processors to
    /// other programs and ease the load on a slow disk.
    #[arg(long, value_name = "N")]
    pub(crate) threads: Option<NonZeroUsize>,
    /// Cancels the run once this many Entries have ended, as Ctrl+C would.
    #[arg(long, hide = true, value_name = "COUNT")]
    pub(crate) cancel_after: Option<usize>,
}
