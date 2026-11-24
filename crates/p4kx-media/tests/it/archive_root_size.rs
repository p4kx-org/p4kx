// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{
    ArchiveProvider, Capability, MediaError, MediaWriter, Projection, Provider, ProviderRegistry,
    RootSource, SourceContext,
};
use serde_json::{Value, json};

use super::build_origin_fixture::{self as fixture, Channel, Input, Run, TestResult};
use super::build_origin_support;

const DATA_SIZE: &str = "/data/size";
const SOURCE_SIZE: &str = "/source/size";

#[test]
fn root_record_sizes_share_the_original_completed_file_length() -> TestResult {
    let channel = Channel::new(Input::Missing, false)?;
    let run = channel.export("root-size", &fixture::options(), &fixture::registry()?)?;
    let expected = channel.archive.file_len();
    assert_eq!(run.root()?.1.pointer(DATA_SIZE), Some(&json!(expected)));
    assert_eq!(
        run.root()?.1.pointer(SOURCE_SIZE),
        Some(&json!(expected)),
        "the source envelope retains the original measured length"
    );

    for path in [DATA_SIZE, SOURCE_SIZE] {
        assert_root_size_origin(&run, path, expected)?;
    }
    let (ordinal, record) = run
        .records
        .iter()
        .enumerate()
        .find(|(_, row)| {
            row.pointer("/subresource/namespace")
                .and_then(Value::as_str)
                == Some("archive-entry")
        })
        .ok_or("expected the original Entry record")?;
    let entry = channel.archive.entries().first().ok_or("fixture Entry")?;
    assert_eq!(
        record.pointer(SOURCE_SIZE),
        Some(&json!(entry.uncompressed_size())),
    );
    let ordinal = ordinal.to_string();
    let field = run
        .fields
        .iter()
        .find(|row| {
            row.pointer("/scope/stream/type").and_then(Value::as_str) == Some("records")
                && row.pointer("/scope/stream/kind").and_then(Value::as_str) == Some("archive")
                && row.pointer("/scope/ordinal").and_then(Value::as_str) == Some(ordinal.as_str())
                && row.get("field").and_then(Value::as_str) == Some(SOURCE_SIZE)
        })
        .ok_or("expected the Entry envelope's size origin")?;
    let contributions = run.contributions(field)?;
    let [used] = contributions.as_slice() else {
        return Err("expected one Entry size contributor".into());
    };
    assert_eq!(
        used.get("scope"),
        Some(&json!("archiveDirectoryMetadata")),
        "Entry size remains an original directory declaration",
    );
    assert_eq!(
        used.pointer("/location/field"),
        Some(&json!("uncompressedSize"))
    );
    assert_eq!(
        run.physical(used)?.get("kind"),
        Some(&json!("archiveEntry"))
    );
    Ok(())
}

#[test]
fn detached_and_modified_root_values_do_not_gain_measured_size_authority() -> TestResult {
    let channel = Channel::new(Input::Missing, false)?;
    let expected = channel.archive.file_len();
    let options = fixture::options();
    for (name, provider, data_size) in [
        ("detached", RootProjection::Detached, expected),
        ("modified", RootProjection::Modified, expected + 1),
    ] {
        let registry = ProviderRegistry::try_new(vec![Box::new(provider)])?;
        let run = channel.export(name, &options, &registry)?;
        assert_eq!(
            run.root()?.1.pointer(DATA_SIZE),
            Some(&json!(data_size)),
            "the provider payload retains its actual value"
        );
        build_origin_support::unknown(&run, "records", DATA_SIZE)?;
        assert_eq!(
            run.root()?.1.pointer(SOURCE_SIZE),
            Some(&json!(expected)),
            "the source envelope retains the original measured length"
        );
        assert_root_size_origin(&run, SOURCE_SIZE, expected)?;
    }
    Ok(())
}

fn assert_root_size_origin(run: &Run, path: &str, expected: u64) -> TestResult {
    let field = run.field("records", path)?;
    assert_eq!(
        field.get("containsEncryptedSource"),
        Some(&Value::Null),
        "the encryption of the entire file stays unknown"
    );
    assert_eq!(
        field.get("sourceSetComplete"),
        Some(&Value::Bool(true)),
        "the actual file read fully accounts for the size"
    );
    let contributions = run.contributions(field)?;
    let [used] = contributions.as_slice() else {
        return Err("expected one completed Archive file length contributor".into());
    };
    assert_eq!(
        used.get("scope"),
        Some(&json!("fileMetadata")),
        "the length comes from the whole file domain"
    );
    assert_eq!(
        used.get("role"),
        Some(&json!("payload")),
        "the measured length contributes the value"
    );
    assert_eq!(
        used.get("effectiveEncryption"),
        Some(&json!("unknown")),
        "the file source keeps unknown encryption"
    );
    assert_eq!(
        used.get("location"),
        Some(&json!({"type": "metadata", "field": "byteLength"})),
        "the selected metadata value is the exact file byte length",
    );
    let source = run.physical(used)?;
    assert_eq!(
        source.get("kind"),
        Some(&json!("archiveFile")),
        "the original whole file owns the size"
    );
    assert_eq!(
        source.pointer("/byteLength/value"),
        Some(&json!(expected.to_string())),
        "the physical file fact reports the same byte count",
    );
    let length = source.get("byteLength").ok_or("Archive file length fact")?;
    assert_eq!(
        length.get("sourceUsesRefs"),
        field.get("sourceUsesRefs"),
        "the payload cites the existing measured file length use"
    );
    Ok(())
}

enum RootProjection {
    Detached,
    Modified,
}

impl Provider for RootProjection {
    fn capability(&self) -> Capability {
        ArchiveProvider.capability()
    }

    fn root(&self, root: &RootSource, writer: &mut MediaWriter<'_>) -> Result<(), MediaError> {
        let projection = match self {
            Self::Detached => serde_json::to_value(root)
                .and_then(serde_json::from_value)
                .map_err(|cause| MediaError::Contract(cause.to_string()))?,
            Self::Modified => {
                let mut projection = root.clone();
                projection.size += 1;
                projection
            }
        };
        ArchiveProvider.root(&projection, writer)
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        ArchiveProvider.source(context, writer)
    }
}
