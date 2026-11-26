// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_audio::{AudioError, Wem};

use crate::file_receipt::{self, StagedReceiptPublication, finish_staged_receipt};
use crate::origin_file_io::StageDisposition;
use crate::output_file::{StagedOutput, read_error, stage_counted_protected, write_error};
use crate::{
    ArchiveWriteProtection, CancelToken, ConflictPolicy, ConversionError, FileReceipt,
    OriginArtifactDigest, OriginDigestWriter, Replaced,
};

use super::{
    OggWrite, SoundEncodeError, SoundOriginInput, SoundOrigins, WavWrite, capture_ogg,
    capture_raw_sound, capture_wav,
};

/// The representation to publish for an already-read WEM.
#[derive(Clone, Copy)]
#[non_exhaustive]
pub enum WemOutput<'sound, 'bytes> {
    /// Copy the supplied payload, including a prefetched prefix.
    Raw(&'bytes [u8]),
    /// Encode the parsed WEM as PCM WAV.
    Wav(&'sound Wem<&'bytes [u8]>),
    /// Rebuild the parsed WEM's Ogg stream.
    Ogg(&'sound Wem<&'bytes [u8]>),
}

/// The destination and write controls for one WEM output.
#[derive(Clone, Copy)]
pub struct WemOutputTarget<'a> {
    path: &'a Path,
    policy: ConflictPolicy,
    protection: Option<&'a ArchiveWriteProtection>,
    cancel: &'a CancelToken,
}

impl<'a> WemOutputTarget<'a> {
    /// Selects the destination, conflict policy, source guard and cancellation token.
    #[must_use]
    pub const fn new(
        path: &'a Path,
        policy: ConflictPolicy,
        protection: Option<&'a ArchiveWriteProtection>,
        cancel: &'a CancelToken,
    ) -> Self {
        Self {
            path,
            policy,
            protection,
            cancel,
        }
    }
}

/// The artifact placement and its separate companion publication result.
#[non_exhaustive]
#[must_use]
pub struct WemOutputPublication {
    /// The written or kept artifact.
    pub artifact: Replaced,
    /// The sibling companion published in this operation.
    pub origin_receipt: Option<PathBuf>,
    /// A failure after the artifact was committed.
    pub receipt_failure: Option<ConversionError>,
}

impl WemOutputPublication {
    const fn kept(len: u64) -> Self {
        Self {
            artifact: Replaced {
                len,
                changed: false,
            },
            origin_receipt: None,
            receipt_failure: None,
        }
    }
}

/// Publishes WEM bytes or their conversion with a companion for the selected input bytes.
/// The companion describes the artifact's payload and conversion fields.
///
/// # Errors
/// Returns an encoding, conflict check or publication failure before artifact placement.
pub fn publish_wem_output(
    output: WemOutput<'_, '_>,
    source: Option<SoundOriginInput<'_>>,
    target: WemOutputTarget<'_>,
    label: impl FnOnce() -> String,
) -> Result<WemOutputPublication, ConversionError> {
    stage_counted_protected(
        target.path,
        target.policy == ConflictPolicy::IfChanged,
        target.protection,
        Some(target.cancel),
        |writer| encode(output, target.path, label, target.cancel, writer),
        WemOutputPublication::kept,
        |(written, digest), _, staged| publish_encoded(staged, digest, &written, source, &target),
    )
}

fn publish_encoded(
    staged: &StagedOutput<'_>,
    digest: OriginArtifactDigest,
    written: &EncodedWem<'_, '_>,
    source: Option<SoundOriginInput<'_>>,
    target: &WemOutputTarget<'_>,
) -> Result<StageDisposition<WemOutputPublication>, ConversionError> {
    let companion = file_receipt::companion_path(target.path)
        .map_err(|source| write_error(target.path, source))?;
    let source = source.filter(|input| !matches!(input.source(), crate::SourceInput::Unknown));
    let origins = written
        .capture(source)
        .map_err(|source| write_error(&companion, source))?;
    let receipt = FileReceipt::for_native_staged_replacement(
        target.path,
        target.protection,
        target.cancel.clone(),
    )
    .map_err(|source| write_error(&companion, source))?;
    let len = digest.byte_length();
    let placed = receipt.publish_staged_with_policy(
        staged,
        digest,
        target.policy,
        || Ok(()),
        || {},
        |writer, binding| origins.write(writer, binding),
    );
    match placed {
        None => {
            let len = fs::symlink_metadata(target.path)
                .map_err(|source| read_error(target.path, source))?
                .len();
            Ok(StageDisposition::Cleanup(WemOutputPublication::kept(len)))
        }
        Some(StagedReceiptPublication::Uncommitted(source)) => {
            Err(write_error(target.path, source))
        }
        Some(StagedReceiptPublication::Committed {
            receipt,
            retirement,
        }) => Ok(StageDisposition::Moved(finish_publication(
            staged,
            companion,
            len,
            (receipt, retirement),
            target,
        ))),
    }
}

fn finish_publication(
    staged: &StagedOutput<'_>,
    companion: PathBuf,
    len: u64,
    (receipt, retirement): (Option<io::Result<()>>, io::Result<()>),
    target: &WemOutputTarget<'_>,
) -> WemOutputPublication {
    let published = receipt.as_ref().is_some_and(Result::is_ok);
    let publication = finish_staged_receipt(receipt, retirement)
        .map_err(|source| write_error(&companion, source));
    let publication = if target.policy == ConflictPolicy::Skip {
        crate::output_cleanup::conversion(
            publication,
            staged.owner(),
            staged.temporary(),
            Some(target.cancel),
        )
    } else {
        publication
    };
    WemOutputPublication {
        artifact: Replaced { len, changed: true },
        origin_receipt: published.then_some(companion),
        receipt_failure: publication.err(),
    }
}

enum EncodedWem<'sound, 'bytes> {
    Raw(u64),
    Wav(&'sound Wem<&'bytes [u8]>, WavWrite),
    Ogg(&'sound Wem<&'bytes [u8]>, OggWrite),
}

impl EncodedWem<'_, '_> {
    fn capture(&self, source: Option<SoundOriginInput<'_>>) -> io::Result<SoundOrigins> {
        match self {
            Self::Raw(length) => capture_raw_sound(source, *length),
            Self::Wav(wem, written) => capture_wav(wem, source, written),
            Self::Ogg(wem, written) => capture_ogg(wem, source, written),
        }
    }

    const fn length(&self) -> u64 {
        match self {
            Self::Raw(length) => *length,
            Self::Wav(_, written) => written.report().size,
            Self::Ogg(_, written) => written.report().size,
        }
    }
}

fn encode<'sound, 'bytes>(
    output: WemOutput<'sound, 'bytes>,
    path: &Path,
    label: impl FnOnce() -> String,
    cancel: &CancelToken,
    writer: &mut dyn Write,
) -> Result<(EncodedWem<'sound, 'bytes>, OriginArtifactDigest), ConversionError> {
    let mut measured = OriginDigestWriter::new(CancellableWriter { writer, cancel });
    let encoded = match output {
        WemOutput::Raw(bytes) => measured
            .write_all(bytes)
            .map(|()| EncodedWem::Raw(bytes.len() as u64))
            .map_err(AudioError::Write),
        WemOutput::Wav(wem) => WavWrite::encode(wem, &mut measured)
            .map(|written| EncodedWem::Wav(wem, written))
            .map_err(SoundEncodeError::into_audio_error),
        WemOutput::Ogg(wem) => OggWrite::encode(wem, &mut measured)
            .map(|written| EncodedWem::Ogg(wem, written))
            .map_err(SoundEncodeError::into_audio_error),
    }
    .map_err(|source| match source {
        AudioError::Write(source) => write_error(path, source),
        source => ConversionError::Audio {
            sound: label(),
            source,
        },
    })?;
    let digest = measured
        .finish()
        .map_err(|source| write_error(path, source))?;
    if digest.byte_length() != encoded.length() {
        return Err(write_error(
            path,
            io::Error::other("audio byte length differs from its write report"),
        ));
    }
    Ok((encoded, digest))
}

struct CancellableWriter<'a> {
    writer: &'a mut dyn Write,
    cancel: &'a CancelToken,
}

impl Write for CancellableWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other("the export was cancelled"));
        }
        self.writer.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}
