// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io;

use p4kx_extraction::{Archive, CancelToken};
use p4kx_media::{
    ArtifactExtension, Capability, MediaError, MediaKind, MediaWriter, Projection, Provider,
    ProviderRegistry, SourceContext, Status, Subresource,
};
use p4kx_testkit::TestEntry;

use super::support::{self, TestResult};

pub(super) const PAYLOAD: &[u8] = b"encoded synthetic payload";

#[derive(Clone, Copy)]
pub(super) enum Mode {
    Success,
    Repeat,
    Empty,
    Io,
    Cancelled,
    IgnoredQuota,
}

struct EncodedProvider {
    mode: Mode,
    cancel: CancelToken,
}

impl Provider for EncodedProvider {
    fn capability(&self) -> Capability {
        Capability::new(
            MediaKind::Texture,
            "synthetic.encoded-output",
            "1.0.0",
            "unknown-length synthetic encoded artifacts",
        )
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let mut artifacts = Vec::new();
        let copies = if matches!(self.mode, Mode::Repeat) {
            2
        } else {
            1
        };
        for _ in 0..copies {
            let artifact = writer
                .converted_artifact(ArtifactExtension::new("custom2")?, |output| {
                    self.encode(context, output)
                })?;
            artifacts.push(artifact);
        }
        writer.record(
            &Subresource::new("synthetic.encoded", vec![0]),
            Status::Complete,
            &artifacts,
            &[],
        )?;
        Ok(Projection::Written)
    }
}

impl EncodedProvider {
    fn encode(
        &self,
        context: &SourceContext<'_>,
        output: &mut dyn io::Write,
    ) -> Result<(), MediaError> {
        if matches!(self.mode, Mode::Empty) {
            return Ok(());
        }
        output.write_all(PAYLOAD).map_err(io_error)?;
        match self.mode {
            Mode::Io => {
                File::open(context.payload.with_extension("missing-encoder-input"))
                    .map_err(io_error)?;
                Err(MediaError::Contract("missing encoder input opened".into()))
            }
            Mode::Cancelled => {
                self.cancel.cancel();
                Ok(())
            }
            Mode::IgnoredQuota => {
                let block = vec![0; 64 << 10];
                for _ in 0..4 {
                    if output.write_all(&block).is_err() {
                        return Ok(());
                    }
                }
                Err(MediaError::Contract(
                    "synthetic output quota was not reached".into(),
                ))
            }
            Mode::Success | Mode::Repeat | Mode::Empty => Ok(()),
        }
    }
}

pub(super) fn registry(mode: Mode, cancel: CancelToken) -> Result<ProviderRegistry, MediaError> {
    ProviderRegistry::try_new(vec![Box::new(EncodedProvider { mode, cancel })])
}

pub(super) fn archive(root: &std::path::Path) -> Result<Archive, Box<dyn std::error::Error>> {
    support::archive_with(root, [TestEntry::stored("source.bin", b"H")])
}

pub(super) fn contains(
    error: &(dyn std::error::Error + 'static),
    predicate: impl Fn(&MediaError) -> bool,
) -> bool {
    let mut current = Some(error);
    while let Some(error) = current {
        if error.downcast_ref::<MediaError>().is_some_and(&predicate) {
            return true;
        }
        current = error.source();
    }
    false
}

pub(super) fn assert_artifact(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    artifact: &p4kx_media::Artifact,
    payload: &[u8],
) -> TestResult {
    use sha2::{Digest as _, Sha256};
    assert_eq!(
        std::fs::read(output.join(&index.artifact_base).join(&artifact.path))?,
        payload,
        "published artifact preserves the exact encoded bytes"
    );
    assert_eq!(
        artifact.size,
        u64::try_from(payload.len())?,
        "measured encoded length"
    );
    assert_eq!(
        artifact.digest.hex,
        p4kx_extraction::Checksum::Sha256(Sha256::digest(payload).into()).hex(),
        "digest covers the actual complete encoded output"
    );
    assert_eq!(
        artifact.digest.scope,
        p4kx_media::ByteScope::ArtifactFile,
        "artifact byte scope"
    );
    assert_eq!(
        artifact.path,
        format!("chunks/{}.custom2", artifact.digest.hex),
        "content-addressed path retains the validated extension"
    );
    Ok(())
}

fn io_error(source: io::Error) -> MediaError {
    MediaError::Io {
        path: "synthetic encoder input".into(),
        source,
    }
}

pub(super) fn assert_manifest(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    converted: &str,
    uses: usize,
) -> TestResult {
    let manifest = std::fs::read_to_string(
        output
            .join(&index.artifact_base)
            .join(&index.artifact_manifest.path),
    )?;
    let manifest: Vec<p4kx_media::Artifact> = manifest
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    assert_eq!(
        manifest
            .iter()
            .filter(|artifact| artifact.path == converted)
            .count(),
        uses,
        "every use references the measured output"
    );
    assert_eq!(
        manifest
            .iter()
            .filter(|artifact| artifact.path.starts_with("raw/"))
            .count(),
        1,
        "raw retention remains independent"
    );
    Ok(())
}
