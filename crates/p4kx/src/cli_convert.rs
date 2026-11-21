// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;
use std::path::PathBuf;

use clap::Args;

use crate::cli_conflict::Conflict;
use crate::cli_output::{AudioOutput, TextureOutput};

/// Arguments of `p4kx convert`.
#[derive(Debug, Args)]
pub(crate) struct ConvertArgs {
    /// The folder of files extracted earlier, e.g. by `p4kx extract` without `--convert`; its
    /// relative paths act as Entry paths, and links in it are never followed.
    pub(crate) folder: PathBuf,
    /// Target folder, created if missing; it must not lie inside the input folder or contain it.
    /// Files already in it are handled by --conflict.
    #[arg(long, value_name = "FOLDER")]
    pub(crate) out: PathBuf,
    /// Converts only the files whose relative path matches this Filter, with the rules of
    /// `p4kx search`. A Filter that matches no file is invalid input.
    #[arg(long, value_name = "FILTER")]
    pub(crate) filter: Option<String>,
    /// Leaves out every file whose relative path this Filter matches, with the rules of
    /// `p4kx extract --exclude`; repeatable. A texture whose header it matches is left out with
    /// its parts; a part it matches alone is not written but still read to build its texture,
    /// which a note on stderr tells. A pair that leaves nothing is invalid input.
    #[arg(long, value_name = "FILTER")]
    pub(crate) exclude: Vec<String>,
    /// What to do with a file that already exists in the target.
    #[arg(long, value_enum, default_value_t = Conflict::Skip)]
    pub(crate) conflict: Conflict,
    /// What to write for a texture `<name>.dds`, its parts `<name>.dds.1` … .N and any alpha tail
    /// `<name>.dds.a`, as `extract --convert --textures` does: `merge` one standard DDS, `png` the
    /// mip --mip as `<name>.png` (a high-range texture as OpenEXR, a cubemap as six faces), `raw`
    /// the files unchanged. A texture whose parts are missing or that cannot be converted is
    /// reported and written unchanged.
    #[arg(
        long,
        value_enum,
        value_delimiter = ',',
        num_args = 1..,
        default_values_t = [TextureOutput::Merge, TextureOutput::Png],
        value_name = "OUTPUTS"
    )]
    pub(crate) textures: Vec<TextureOutput>,
    /// The mip level exported as PNG or OpenEXR, 0 being the largest; a texture with fewer
    /// levels exports its smallest.
    #[arg(long, default_value_t = 0, value_name = "LEVEL")]
    pub(crate) mip: u32,
    /// What to write for a WEM sound `<name>.wem` and for each WEM a soundbank `<name>.bnk` embeds,
    /// as `extract --convert --audio` does.
    #[arg(
        long,
        value_enum,
        default_value_t = AudioOutput::Ogg,
        value_name = "FORMAT"
    )]
    pub(crate) audio: AudioOutput,
    /// Also writes the files that have nothing to convert, unchanged. Without it they are left
    /// out and counted as not converted.
    #[arg(long)]
    pub(crate) copy_other: bool,
    /// The most threads the conversion uses; by default one per logical processor.
    #[arg(long, value_name = "N")]
    pub(crate) threads: Option<NonZeroUsize>,
    /// Cancels the run once this many files have ended, as Ctrl+C would.
    #[arg(long, hide = true, value_name = "COUNT")]
    pub(crate) cancel_after: Option<usize>,
}
