// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_media::{Identity, ShaderPayload, ShaderProvider, Status};
use p4kx_testkit::{TestContainer, TestEntry};

use super::shader_support::{self as support, TestResult};

const FIRST: &str = "Version(1)\nUsesCommonGlobalFlags\nProperty { Name=%A; Mask(0x20000000000001); Hidden; Unknown { Nested(\"})\") } }\nProperty { Name=%A; Name(); Hidden(); DependencySet($A); DependencySet=($B|$C); }";
const SECOND: &str = "\u{feff}Version = synthetic\r\nMystery { Child = %X; }\r\nProperty { Description(\"工具\"); Mask(0); }";

#[test]
fn nested_sources_keep_ordered_identity_declarations_spans_and_deterministic_rows() -> TestResult {
    let folder = tempfile::tempdir()?;
    let container = TestContainer::new()
        .entry(TestEntry::stored("Shaders/Synthetic.EXT", FIRST.as_bytes()))
        .entry(TestEntry::stored("Shaders/Other.ext", SECOND.as_bytes()));
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("synthetic.pak", container.build())],
    )?;
    let registry = support::registry(ShaderProvider::default())?;
    let mut streams = Vec::new();
    for name in ["first", "second"] {
        let output = folder.path().join(name);
        let index = support::published(p4kx_media::export_with(
            &archive,
            &output,
            &support::options(),
            &registry,
        )?)?;
        let records = support::records(&output, &index)?;
        verify_sources(&output, &index, &records)?;
        streams.push(fs::read(
            output.join(&index.artifact_base).join("shader.jsonl"),
        )?);
    }
    assert_eq!(
        streams.first(),
        streams.get(1),
        "repeated exports emit deterministic shader rows"
    );
    Ok(())
}

fn verify_sources(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    records: &[p4kx_media::MediaRecord],
) -> TestResult {
    assert_eq!(
        records.len(),
        22,
        "two independent source definitions contain exactly twenty declarations"
    );
    let mut definitions = 0;
    let mut properties = 0;
    let mut counted_declarations = 0;
    let mut declaration_rows = 0;
    for record in records {
        match verify_record(output, index, record)? {
            ShaderPayload::Definition(data) => {
                definitions += 1;
                counted_declarations += data.counts.declarations;
            }
            ShaderPayload::Property(_) => {
                properties += 1;
                declaration_rows += 1;
            }
            ShaderPayload::Declaration(_) => {
                declaration_rows += 1;
            }
            _ => return Err("unexpected source payload family".into()),
        }
    }
    assert_eq!(
        (definitions, properties),
        (2, 3),
        "definition and Property counts are distinct"
    );
    assert_eq!(
        counted_declarations, declaration_rows,
        "every counted declaration has exactly one row"
    );
    assert_eq!(
        declaration_rows, 20,
        "independently authored fixture count detects shared reader/projector omissions"
    );
    Ok(())
}

fn verify_record(
    output: &std::path::Path,
    index: &p4kx_media::Index,
    record: &p4kx_media::MediaRecord,
) -> Result<ShaderPayload, Box<dyn std::error::Error>> {
    use sha2::Digest as _;

    assert_eq!(
        record.status,
        Status::Complete,
        "supported source inventory has complete coverage"
    );
    let Identity::Entry { containers, .. } = &record.source.identity else {
        return Err("shader Entry identity".into());
    };
    assert_eq!(containers.len(), 1, "nested .pak identity remains present");
    assert_eq!(
        containers.first().ok_or("container")?.path,
        "synthetic.pak",
        "container spelling is retained"
    );
    let raw = record.source.raw.as_ref().ok_or("shader raw source")?;
    let bytes = fs::read(output.join(&index.artifact_base).join(&raw.path))?;
    let digest = p4kx_extraction::Checksum::Sha256(sha2::Sha256::digest(&bytes).into()).hex();
    assert_eq!(
        raw.digest.hex, digest,
        "raw source hash independently agrees with usable retained bytes"
    );
    let text = std::str::from_utf8(&bytes)?;
    let payload: ShaderPayload = serde_json::from_value(record.data.clone())?;
    match &payload {
        ShaderPayload::Definition(data) => assert_eq!(
            data.span.end_exclusive, raw.size,
            "source span reaches the original Entry end"
        ),
        ShaderPayload::Property(data) => verify_declaration(text, &data.declaration)?,
        ShaderPayload::Declaration(data) => verify_declaration(text, data)?,
        _ => return Err("unexpected source payload family".into()),
    }
    Ok(payload)
}

fn verify_declaration(text: &str, declaration: &p4kx_media::ShaderDeclaration) -> TestResult {
    for token in [&declaration.name, &declaration.value, &declaration.literal]
        .into_iter()
        .flatten()
    {
        let start = usize::try_from(token.span.start)?;
        let end = usize::try_from(token.span.end_exclusive)?;
        assert_eq!(
            text.get(start..end),
            Some(token.text.as_str()),
            "every raw token agrees with its authoritative original Entry range"
        );
    }
    assert!(
        declaration.span.start <= declaration.span.end_exclusive,
        "source construct range is ordered"
    );
    assert!(
        declaration.span.end_exclusive <= u64::try_from(text.len())?,
        "source construct stays within retained Entry bytes"
    );
    Ok(())
}

#[test]
fn same_inner_path_in_two_containers_remains_two_definitions() -> TestResult {
    let folder = tempfile::tempdir()?;
    let nested = TestContainer::new().entry(TestEntry::stored(
        "Shaders/Same.ext",
        b"Property { Name=%A; }",
    ));
    let archive = support::archive(
        folder.path(),
        [
            TestEntry::stored("first.pak", nested.build()),
            TestEntry::stored("second.pak", nested.build()),
        ],
    )?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(ShaderProvider::default())?,
    )?)?;
    let identities: Vec<_> = support::records(&output, &index)?
        .into_iter()
        .filter(|record| record.subresource.namespace == "shader-definition")
        .map(|record| record.source.identity)
        .collect();
    assert_eq!(identities.len(), 2, "same inner spelling is not merged");
    assert_ne!(
        identities.first(),
        identities.get(1),
        "physical container chains distinguish identical payload bytes"
    );
    Ok(())
}

#[test]
fn compiled_cache_inventory_never_counts_decoded_definitions_or_properties() -> TestResult {
    let folder = tempfile::tempdir()?;
    let entries = [
        "compiled.CFIB",
        "compiled.cfxb",
        "compiled.shcb",
        "unknown.dpsi",
        "unknown.fx",
        "ext",
        "source.mtl",
    ]
    .into_iter()
    .map(|path| TestEntry::stored(path, b"synthetic opaque bytes"));
    let archive = support::archive(folder.path(), entries)?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(ShaderProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    assert_eq!(
        records.len(),
        3,
        "only the three exact compiled-cache extensions are inventoried"
    );
    for record in records {
        assert_eq!(
            record.subresource.namespace, "shader-cache-inventory",
            "cache does not masquerade as source declaration"
        );
        assert_eq!(
            record.status,
            Status::Complete,
            "inventory is complete while semantic decoding stays unsupported"
        );
        let ShaderPayload::CompiledCache(data) = serde_json::from_value(record.data)? else {
            return Err("compiled cache payload".into());
        };
        assert_eq!(
            (data.compiled_caches, data.definitions, data.properties),
            (1, 0, 0),
            "counting units remain separate"
        );
        assert!(
            record.source.raw.is_some(),
            "opaque cache bytes remain usable"
        );
    }
    let kind = index.kinds.first().ok_or("shader summary")?;
    assert_eq!(
        (
            kind.counts.source_entries.written,
            kind.counts.source_entries.skipped
        ),
        (3, 4),
        "inventoried cache and unrelated inputs remain distinct"
    );
    Ok(())
}
