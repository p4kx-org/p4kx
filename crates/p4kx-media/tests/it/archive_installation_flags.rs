// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::Archive;
use p4kx_media::{
    ArchiveProvider, Capability, ExportOutcome, MediaError, MediaWriter, Projection, Provider,
    ProviderRegistry, RootSource, SourceContext,
};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::{Value, json};

use super::build_origin_fixture::{self as fixture, Channel, Input, TestResult};
use super::build_origin_support;

const FLAGS: &str = "/data/installation_flags";
const TEST_FLAGS: u8 = 0x80;
const TRAILER_BYTES: usize = 175;
const FLAGS_IN_TRAILER: usize = 104;

#[test]
fn v2_root_installation_flags_cite_one_original_directory_byte() -> TestResult {
    let (channel, offset) = v2()?;
    let options = fixture::options();
    let registry = fixture::registry()?;
    let run = channel.export("media", &options, &registry)?;
    assert_eq!(
        run.root()?.1.pointer(FLAGS),
        Some(&json!(TEST_FLAGS)),
        "the payload retains the original flags byte"
    );
    let field = run.field("records", FLAGS)?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Bool(false))
    );
    assert_eq!(field.get("sourceSetComplete"), Some(&Value::Bool(true)));
    let contributions = run.contributions(field)?;
    let [used] = contributions.as_slice() else {
        return Err("expected one installation flags contributor".into());
    };
    assert_eq!(used.get("role"), Some(&json!("payload")));
    assert_eq!(used.get("effectiveEncryption"), Some(&json!("unencrypted")));
    assert_eq!(
        used.get("location"),
        Some(&json!({
            "type": "subresource",
            "namespace": "archiveV2Trailer",
            "indices": [offset.to_string(), (offset + 1).to_string()],
            "field": "flags"
        })),
        "the original range is half-open and covers the flags alone"
    );
    assert_eq!(
        run.physical(used)?.get("kind"),
        Some(&json!("archiveDirectory"))
    );
    assert!(matches!(
        p4kx_media::export_with(&channel.archive, &run.output, &options, &registry)?,
        ExportOutcome::Skipped { .. }
    ));
    Ok(())
}

#[test]
fn missing_and_detached_installation_flags_keep_unknown_origin() -> TestResult {
    let v1 = Channel::new(Input::Missing, false)?;
    let options = fixture::options();
    let absent = v1.export("absent", &options, &fixture::registry()?)?;
    assert_eq!(absent.root()?.1.pointer(FLAGS), Some(&Value::Null));
    build_origin_support::unknown(&absent, "records", FLAGS)?;

    let (channel, _) = v2()?;
    let registry = ProviderRegistry::try_new(vec![Box::new(DetachedArchiveProvider)])?;
    let detached = channel.export("detached", &options, &registry)?;
    assert_eq!(detached.root()?.1.pointer(FLAGS), Some(&json!(TEST_FLAGS)));
    build_origin_support::unknown(&detached, "records", FLAGS)?;
    Ok(())
}

fn v2() -> TestResult<(Channel, usize)> {
    let folder = tempfile::tempdir()?;
    let mut bytes = ArchiveBuilder::v2()
        .entry(TestEntry::stored("Data/a.bin", b"synthetic payload"))
        .build();
    let offset = bytes.len() - TRAILER_BYTES + FLAGS_IN_TRAILER;
    *bytes
        .get_mut(offset)
        .ok_or("synthetic v2 installation byte")? = TEST_FLAGS;
    let path = folder.path().join("Data.p4k");
    fs::write(&path, bytes)?;
    let archive = Archive::open(path)?;
    Ok((Channel { folder, archive }, offset))
}

struct DetachedArchiveProvider;

impl Provider for DetachedArchiveProvider {
    fn capability(&self) -> Capability {
        ArchiveProvider.capability()
    }

    fn root(&self, root: &RootSource, writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        let detached = serde_json::to_value(root)
            .and_then(serde_json::from_value)
            .map_err(|cause| MediaError::Contract(cause.to_string()))?;
        ArchiveProvider.root(&detached, writer)
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        ArchiveProvider.source(context, writer)
    }
}
