// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;

use p4kx_extraction::{ArchiveWatch, Checksum, Exclusion};
use sha2::{Digest as _, Sha256};

use crate::checkpoint::IO_BLOCK_BYTES;
use crate::required_input_fault_support::{self as support, Inventory, TestResult};
use crate::{
    Artifact, ArtifactExtension, ByteOrder, Capability, ChunkLayout, DataType, MediaError,
    MediaKind, MediaWriter, Source, Status, Subresource,
};

#[test]
fn unknown_output_writes_are_bounded_and_are_not_mislabelled_as_small_typed_chunks() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, _| {
            writer.limits.chunk_bytes = 1;
            let bytes = vec![0x5a; IO_BLOCK_BYTES + 17];
            let artifact =
                writer.converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    let written = output.write(&bytes).map_err(io_error)?;
                    assert_eq!(written, IO_BLOCK_BYTES, "one actual file write is bounded");
                    output
                        .write_all(bytes.get(written..).ok_or_else(|| {
                            MediaError::Contract("remaining synthetic bytes".into())
                        })?)
                        .map_err(io_error)?;
                    output.flush().map_err(io_error)
                })?;
            assert_eq!(fs::read(writer.root.join(&artifact.path))?, bytes);
            assert_eq!(artifact.size, u64::try_from(bytes.len())?);
            assert_eq!(
                artifact.digest.hex,
                Checksum::Sha256(Sha256::digest(&bytes).into()).hex()
            );
            assert_eq!(
                fs::read_dir(writer.root.join("chunks"))?
                    .collect::<Result<Vec<_>, _>>()?
                    .len(),
                1
            );
            Ok(())
        },
    )
}

#[test]
fn metadata_and_typed_chunk_accounting_survive_shared_placement_and_conversion_dedup() -> TestResult
{
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, _| {
            let layout = || {
                ChunkLayout::new(
                    DataType::U8,
                    ByteOrder::Little,
                    4,
                    "synthetic original byte order",
                )
            };
            let metadata = writer.metadata_chunk(layout(), &mut b"data".as_slice())?;
            let original = writer
                .kinds
                .first()
                .ok_or("provider summary")?
                .counts
                .converted_outputs;
            assert_eq!(
                (original.enumerated, original.written, original.skipped),
                (0, 0, 0)
            );
            let chunk = writer.chunk(layout(), &mut b"data".as_slice())?;
            assert_eq!(metadata.artifact.path, chunk.artifact.path);
            for _ in 0..2 {
                let before = writer.remaining_output_bytes();
                let artifact = writer
                    .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                        output.write_all(b"data").map_err(io_error)
                    })?;
                let manifest_bytes = u64::try_from(serde_json::to_vec(&artifact)?.len())? + 1;
                assert_eq!(
                    before - writer.remaining_output_bytes(),
                    4 + manifest_bytes,
                    "repeated physical encoding work is charged even when content is kept"
                );
            }
            let converted = writer
                .kinds
                .first()
                .ok_or("provider summary")?
                .counts
                .converted_outputs;
            assert_eq!(
                (converted.enumerated, converted.written, converted.skipped),
                (3, 1, 2)
            );
            Ok(())
        },
    )
}

fn io_error(source: io::Error) -> MediaError {
    MediaError::Io {
        path: "synthetic encoder".into(),
        source,
    }
}

#[test]
fn failed_manifest_admission_returns_no_artifact_and_increments_no_converted_count() -> TestResult {
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |_context, writer, _, _| {
            let admitted = writer.limits.output_bytes - writer.remaining_output_bytes();
            writer.limits.output_bytes = admitted + 4;
            let failure = writer
                .converted_artifact(ArtifactExtension::new("bin")?, |output| {
                    output.write_all(b"data").map_err(io_error)
                })
                .err()
                .ok_or("manifest quota failure returned an artifact")?;
            assert!(matches!(
                failure,
                MediaError::Limit {
                    budget: "output_bytes",
                    ..
                }
            ));
            let counts = writer
                .kinds
                .first()
                .ok_or("provider summary")?
                .counts
                .converted_outputs;
            assert_eq!(
                (counts.enumerated, counts.written, counts.skipped),
                (0, 0, 0)
            );
            assert_eq!(
                fs::read_dir(writer.root.join("chunks"))?
                    .collect::<Result<Vec<_>, _>>()?
                    .len(),
                1,
                "placed bytes remain owned by the unpublished generation cleanup"
            );
            Ok(())
        },
    )
}

#[test]
fn actual_manifest_file_io_runs_the_owning_postcheckpoint_before_returning_success() -> TestResult {
    watch_stream(StreamOwner::Artifacts)
}

#[test]
fn actual_source_stream_file_io_runs_the_owning_postcheckpoint_before_success() -> TestResult {
    watch_stream(StreamOwner::Sources)
}

#[test]
fn actual_record_stream_file_io_runs_the_owning_postcheckpoint_before_success() -> TestResult {
    watch_stream(StreamOwner::Records)
}

#[derive(Clone, Copy)]
enum StreamOwner {
    Artifacts,
    Sources,
    Records,
}

fn watch_stream(owner: StreamOwner) -> TestResult {
    use crate::staged_directory::StagedDirectory;
    use std::time::Duration;
    support::with_fixture(
        support::entries(),
        Inventory::Intact,
        &Exclusion::default(),
        |context, _writer, checkpoint, folder| {
            let stage = StagedDirectory::create(folder, ".stream-postcheck-", checkpoint)?;
            let capability = Capability::new(
                MediaKind::Texture,
                "synthetic.stream-postcheck",
                "1.0.0",
                "actual source and record stream I/O",
            );
            let mut writer = MediaWriter::new(
                stage.permit(),
                vec![capability],
                crate::Limits::default(),
                checkpoint,
                false,
            )?;
            writer.begin(0, context.source.clone())?;
            let stream_watch = ArchiveWatch::of_file(&stage.path().join(stream_path(owner)))?
                .with_interval(Duration::ZERO);
            writer.checkpoint.watch = &stream_watch;
            let result = assert_stream_postcheck(&mut writer, context.source, owner);
            drop(writer);
            stage.finish(result.map_err(|cause| MediaError::Io {
                path: "synthetic stream postcheck".into(),
                source: io::Error::other(cause),
            }))?;
            Ok(())
        },
    )
}

fn stream_path(owner: StreamOwner) -> &'static str {
    match owner {
        StreamOwner::Artifacts => "artifacts.jsonl",
        StreamOwner::Sources => "sources.jsonl",
        StreamOwner::Records => "texture.jsonl",
    }
}

fn assert_stream_postcheck(
    writer: &mut MediaWriter<'_>,
    source: &Source,
    owner: StreamOwner,
) -> TestResult {
    const MAX_ROWS: usize = 1_024;
    let digest = crate::Digest {
        algorithm: crate::Algorithm::Sha256,
        scope: crate::ByteScope::ArtifactFile,
        hex: Checksum::Sha256(Sha256::digest(b"").into()).hex(),
    };
    let artifact = Artifact {
        path: format!("chunks/{}", digest.hex),
        size: 0,
        digest,
    };
    fs::create_dir(writer.root.join("chunks"))?;
    fs::write(writer.root.join(&artifact.path), b"")?;
    for _ in 0..MAX_ROWS {
        let result = match owner {
            StreamOwner::Artifacts => writer.artifact(&artifact),
            StreamOwner::Sources => writer.retained(source),
            StreamOwner::Records => writer.record(
                &Subresource::new("synthetic.postcheck", vec![0]),
                Status::Complete,
                &0_u8,
                &[],
            ),
        };
        if fs::metadata(writer.root.join(stream_path(owner)))?.len() > 0 {
            assert!(
                matches!(result, Err(MediaError::Changed(_))),
                "actual buffered stream file I/O must run the owning checkpoint before returning success"
            );
            return Ok(());
        }
        result?;
    }
    Err("synthetic stream never performed actual file I/O".into())
}
