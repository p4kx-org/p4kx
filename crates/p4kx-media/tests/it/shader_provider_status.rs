// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{ShaderPayload, ShaderProvider, Status};
use p4kx_testkit::TestEntry;

use super::shader_support::{self as support, TestResult};

#[test]
fn unsupported_mask_radix_expression_and_width_remain_complete_raw_inventory() -> TestResult {
    let folder = tempfile::tempdir()?;
    let source = b"Property { Mask(17); Mask=($A|$B); Mask(0x10000000000000000); Mask(); Mask; }";
    let archive = support::archive(folder.path(), [TestEntry::stored("source.ext", source)])?;
    let output = folder.path().join("out");
    let index = support::published(p4kx_media::export_with(
        &archive,
        &output,
        &support::options(),
        &support::registry(ShaderProvider::default())?,
    )?)?;
    let records = support::records(&output, &index)?;
    let mut states = Vec::new();
    for record in records {
        assert_eq!(
            record.status,
            Status::Complete,
            "unclaimed general mask interpretation is not falsely diagnosed as lost source data"
        );
        if let ShaderPayload::Declaration(data) = serde_json::from_value(record.data)?
            && let p4kx_media::ShaderMeaning::Mask { interpretation } = data.meaning
        {
            assert_eq!(
                record.issues.len(),
                1,
                "each noninterpretation state has an explicit inventory diagnosis"
            );
            assert_eq!(
                record.issues.first().ok_or("issue")?.severity,
                p4kx_media::Severity::Info,
                "unsupported profile does not claim malformed engine semantics"
            );
            states.push(interpretation);
        }
    }
    assert_eq!(
        states,
        [
            p4kx_media::ShaderMask::UnknownRadix,
            p4kx_media::ShaderMask::UnsupportedExpression,
            p4kx_media::ShaderMask::Overflow,
            p4kx_media::ShaderMask::EmptyValue,
            p4kx_media::ShaderMask::MissingValue
        ],
        "all distinct authored/noninterpretation states survive"
    );
    Ok(())
}

#[test]
fn malformed_hex_refuses_normal_publication_and_is_partial_in_diagnostic_output() -> TestResult {
    let folder = tempfile::tempdir()?;
    let archive = support::archive(
        folder.path(),
        [TestEntry::stored("source.ext", b"Property { Mask(0xGG); }")],
    )?;
    let registry = support::registry(ShaderProvider::default())?;
    let normal = folder.path().join("normal");
    let error = p4kx_media::export_with(&archive, &normal, &support::options(), &registry)
        .err()
        .ok_or("malformed hex was published as complete")?;
    assert!(
        support::projection_loss(&error),
        "literal failure follows the existing source-wrapped projection-loss path"
    );
    assert!(
        !normal.join("media-index.json").exists(),
        "failed normal export publishes no complete index"
    );
    let output = folder.path().join("diagnostic");
    let options = p4kx_media::ExportOptions {
        diagnostic: true,
        ..support::options()
    };
    let diagnostic = support::incomplete(p4kx_media::export_with(
        &archive, &output, &options, &registry,
    )?)?;
    let partial: Vec<_> = support::records(&diagnostic.directory, &diagnostic.index)?
        .into_iter()
        .filter(|record| record.status == Status::Partial)
        .collect();
    assert_eq!(
        partial.len(),
        1,
        "only the malformed Mask occurrence loses its promised literal interpretation"
    );
    assert_eq!(
        partial
            .first()
            .ok_or("partial mask")?
            .issues
            .first()
            .ok_or("diagnosis")?
            .code,
        "shader.mask.malformed",
        "supported-profile failure is explicit"
    );
    Ok(())
}

#[test]
fn invalid_encoding_truncation_and_domain_limit_keep_raw_failed_source_and_unknown_counts()
-> TestResult {
    for (name, source, limit, reason) in [
        (
            "encoding",
            b"Property{\xff}".as_slice(),
            1024,
            "unknown-encoding",
        ),
        (
            "truncated",
            b"Property { Name(%A)".as_slice(),
            1024,
            "unterminated",
        ),
        ("bounded", b"Property { Name(%A) }".as_slice(), 8, "limit"),
    ] {
        let folder = tempfile::tempdir()?;
        let archive = support::archive(folder.path(), [TestEntry::stored("source.ext", source)])?;
        let provider = ShaderProvider::new_with_limits(p4kx_shader::Limits {
            source_bytes: limit,
            ..p4kx_shader::Limits::default()
        });
        let output = folder.path().join(name);
        let options = p4kx_media::ExportOptions {
            diagnostic: true,
            ..support::options()
        };
        let diagnostic = support::incomplete(p4kx_media::export_with(
            &archive,
            &output,
            &options,
            &support::registry(provider)?,
        )?)?;
        verify_failed_definition(&diagnostic, source, reason)?;
    }
    Ok(())
}

fn verify_failed_definition(
    diagnostic: &support::Diagnostic,
    source: &[u8],
    reason: &str,
) -> TestResult {
    let records = support::records(&diagnostic.directory, &diagnostic.index)?;
    assert_eq!(
        records.len(),
        1,
        "unusable syntax has one failed source row, not invented partial properties"
    );
    let record = records.first().ok_or("failed source")?;
    assert_eq!(
        record.status,
        Status::Failed,
        "failed structured reading is not healthy empty inventory"
    );
    assert_eq!(
        record.subresource.namespace, "shader-definition-failure",
        "failure has an explicit namespace"
    );
    assert_eq!(
        record
            .data
            .pointer("/payload/failure/reason")
            .and_then(serde_json::Value::as_str),
        Some(reason),
        "encoding, structure and domain bounds retain distinct typed diagnoses"
    );
    jsonschema::validator_for(&p4kx_media::shader_record_schema()?)?
        .validate(&serde_json::to_value(record)?)
        .map_err(|error| error.to_string())?;
    assert!(
        record
            .data
            .pointer("/payload/counts")
            .is_some_and(serde_json::Value::is_null),
        "unread counts are null instead of zeros"
    );
    let raw = record
        .source
        .raw
        .as_ref()
        .ok_or("retained failed raw source")?;
    assert_eq!(
        std::fs::read(
            diagnostic
                .directory
                .join(&diagnostic.index.artifact_base)
                .join(&raw.path)
        )?,
        source,
        "failure never substitutes or drops source bytes"
    );
    Ok(())
}
