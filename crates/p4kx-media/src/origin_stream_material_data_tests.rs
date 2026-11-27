// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use p4kx_extraction::{Archive, ArchiveWatch, ArchiveWriteProtection, CancelToken};
use p4kx_extraction::{FieldPath, InputScope, SourceUseRole, SourceUses};
use p4kx_testkit::{ArchiveBuilder, TestEntry, test_key};
use serde_json::{Value, json};

use crate::artifact::StreamObserver;
use crate::checkpoint::Checkpoint;
use crate::origin_capture::{OriginCapture, OriginSite};
use crate::origin_contract::{OriginProfile, OriginStream};
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_resolver::{BuiltinOrigins, OriginResolver};
use crate::origin_subject::OriginSubject;
use crate::{
    Algorithm, ByteScope, Digest, Limits, MaterialProvider, MediaKind, MediaWriter, Provider,
    Source, source,
};

type Streams = BTreeMap<String, Vec<u8>>;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[derive(Default)]
struct Sink(RefCell<Streams>);

impl StreamObserver for Sink {
    fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()> {
        self.0
            .borrow_mut()
            .entry(relative.to_owned())
            .or_default()
            .extend_from_slice(bytes);
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum Aggregation {
    Fresh,
    Reused,
}

#[test]
fn material_data_aggregate_memo_preserves_stream_bytes_and_existing_capture_cuts() -> TestResult {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("public.mtl", b"public"))
        .entry(TestEntry::stored("encrypted.mtl", b"encrypted").encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    for (budget, cut) in [(1 << 20, false), (256, true)] {
        let baseline = render(&archive, folder.path(), budget, Aggregation::Fresh)?;
        let memo = render(&archive, folder.path(), budget, Aggregation::Reused)?;
        assert_eq!(
            memo, baseline,
            "field rows, factored sets and every source and use stay byte-identical"
        );
        check_budget(&memo, cut)?;
    }
    Ok(())
}

fn render(archive: &Archive, root: &Path, budget: u64, mode: Aggregation) -> TestResult<Streams> {
    let sink = Sink::default();
    let cancel = CancelToken::default();
    let watch = ArchiveWatch::new(archive);
    let protection = ArchiveWriteProtection::new(archive.path(), cancel.flag())?;
    let mut writer = MediaWriter::observed(
        root,
        vec![MaterialProvider::default().capability()],
        Limits::default(),
        Checkpoint {
            cancel: &cancel,
            watch: &watch,
            protection: &protection,
        },
        false,
        &sink,
    )?;
    writer.origins.observe_root(archive, archive.file_len())?;
    let [clear, encrypted] = archive.entries() else {
        return Err("two distinct physical inputs".into());
    };
    let source = source::entry(
        &Digest {
            algorithm: Algorithm::Sha256,
            scope: ByteScope::RootArchiveFile,
            hex: "0".repeat(64),
        },
        archive,
        clear,
        &[],
    )?;
    let clear = writer.origins.input(archive, clear)?;
    let encrypted = writer.origins.input(archive, encrypted)?;
    emit(&mut writer, &source, &clear, &encrypted, budget, mode)?;
    writer.finish()?;
    Ok(sink.0.into_inner())
}

fn emit(
    writer: &mut MediaWriter<'_>,
    source: &Source,
    clear: &MediaOriginInput,
    encrypted: &MediaOriginInput,
    detail_bytes: u64,
    mode: Aggregation,
) -> TestResult {
    let prepared = prepare(writer, clear, encrypted)?;
    let resolver = OriginResolver {
        owner: &writer.origins.owner,
        root: writer.origins.root.as_ref(),
        source: Some(clear),
        source_prefix: "/source",
        subject: OriginSubject::Record(source),
        artifact: None,
        builtin: prepared.builtin(),
        subresource: None,
        detail_bytes,
    };
    let payload =
        serde_json::to_vec(&json!({"data": {"payload": {"items": [1, 2, 3]}}, "source": source}))?;
    let stream = OriginStream::Records {
        kind: MediaKind::Material,
    };
    let profile = OriginProfile::MaterialPart;
    match mode {
        Aggregation::Fresh => writer.origin_transport.emit_resolved(
            stream,
            profile,
            &payload,
            &resolver,
            Some(&prepared),
            None,
        )?,
        Aggregation::Reused => {
            writer
                .origin_transport
                .emit(stream, profile, &payload, &resolver, Some(&prepared))?;
        }
    }
    Ok(prepared.require_consumed()?)
}

fn prepare(
    writer: &MediaWriter<'_>,
    clear: &MediaOriginInput,
    encrypted: &MediaOriginInput,
) -> TestResult<PreparedRecordOrigins> {
    let mut prepared = writer.record_origins();
    prepared.set_builtin(BuiltinOrigins {
        profile: OriginProfile::MaterialPart,
        input: clear.clone(),
        scope: InputScope::Payload,
    })?;
    let owner = &clear.owner;
    let first = proof(owner, &[(clear, "name"), (encrypted, "value")])?.share_uses();
    let second = proof(owner, &[(clear, "slot"), (encrypted, "lookup")])?.share_uses();
    let long = "long-authored-coordinate".repeat(128);
    for (path, evidence) in [
        ("/data", OriginContributions::independent(owner)),
        ("/data/payload", OriginContributions::independent(owner)),
        ("/data/payload/items/0", first),
        ("/data/payload/items/1", second),
        (
            "/data/payload/items/2",
            proof(owner, &[(encrypted, &long)])?,
        ),
    ] {
        prepared.insert_subtree(&FieldPath::parse(path)?, evidence, writer)?;
    }
    Ok(prepared)
}

fn proof(
    owner: &SourceUses,
    inputs: &[(&MediaOriginInput, &str)],
) -> TestResult<OriginContributions> {
    let mut capture = OriginCapture::with_limit(owner, 1 << 20);
    for &(input, field) in inputs {
        capture.add_original(
            input,
            InputScope::Payload,
            OriginSite::new("materialXmlAttribute", &[1], field),
            SourceUseRole::Payload,
        )?;
    }
    Ok(capture.finish())
}

fn check_budget(streams: &Streams, cut: bool) -> TestResult {
    let bytes = streams
        .get("field-origins.jsonl")
        .ok_or("field origin stream")?;
    let rows = bytes
        .split(|&byte| byte == b'\n')
        .filter(|line| !line.is_empty())
        .map(serde_json::from_slice::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    let data = rows
        .iter()
        .find(|row| row["field"] == "/data")
        .ok_or("aggregated data field")?;
    assert_eq!(
        data["containsEncryptedSource"].as_bool(),
        Some(true),
        "an observed encrypted contributor survives"
    );
    assert_eq!(
        data["sourceSetComplete"], !cut,
        "the low budget cuts the aggregate"
    );
    assert_eq!(
        data.get("sourceUseSetRef").is_some(),
        !cut,
        "an admitted first shared set retains its wire identity"
    );
    if cut {
        assert_eq!(
            data["cuts"],
            json!(["fieldUses"]),
            "the actual field capture budget supplies the cut"
        );
        assert_eq!(
            data["sourceUsesRefs"].as_array().map(Vec::len),
            Some(1),
            "the low budget preserves only the selected opaque witness"
        );
    }
    Ok(())
}
