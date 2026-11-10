// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_extraction::model::MaterialSourceLimits;
use p4kx_extraction::{Archive, ConflictPolicy, OpenOptions};
use p4kx_testkit::{ArchiveBuilder, TestEntry};
use serde_json::Value;

use super::material_origin_support::{self as support, Fixture, MTL, Storage, TestResult};

#[test]
fn material_origin_site_cut_retains_encryption_and_does_not_starve_other_fields() -> TestResult {
    let mut source = String::from("<Material Name='retained'><MatLayers>");
    for _ in 0..2048 {
        source.push_str("<Layer PaletteTint='bad'/>");
    }
    source.push_str("</MatLayers></Material>");
    let mut limits = MaterialSourceLimits::default();
    limits.text_bytes = 1;
    let mut options = support::options();
    options.limits.record_bytes = 128 << 10;
    let fixture = Fixture::with(
        [support::material(
            MTL,
            source.as_bytes(),
            Storage::Encrypted,
        )],
        options,
        limits,
    )?;
    let palette = "/data/payload/materials/0/interpreted_surface/palette";
    fixture.marker(MTL, palette, Some(true))?;
    assert_eq!(
        fixture.field(MTL, palette)?.get("sourceSetComplete"),
        Some(&Value::Bool(false)),
        "the actual optional source-site cut is explicit"
    );
    assert!(
        fixture.contributions(MTL, palette)?.iter().any(|used| used
            .pointer("/location/type")
            .and_then(Value::as_str)
            == Some("opaque")
            && used.get("effectiveEncryption").and_then(Value::as_str) == Some("encrypted")),
        "the original encrypted witness survives unavailable coordinates"
    );
    let name = "/data/payload/materials/0/name";
    fixture.marker(MTL, name, Some(true))?;
    assert_eq!(
        fixture.field(MTL, name)?.get("sourceSetComplete"),
        Some(&Value::Bool(true)),
        "one field's cut does not consume another field's capture allowance"
    );
    fixture.independent(MTL, "/data/payload/materials/0/interpreted_surface/tinted")
}

#[test]
fn unchanged_material_payload_admits_its_exact_original_record_bound() -> TestResult {
    const NAME_BYTES: usize = 16 << 10;
    let source = format!("<Material Name='{}'/>", "x".repeat(NAME_BYTES));
    let fixture = Fixture::new([support::material(MTL, source.as_bytes(), Storage::Plain)])?;
    let original = fs::read(fixture.base.join("material.jsonl"))?;
    let bound = u64::try_from(original.len())?;
    let below = bound.checked_sub(1).ok_or("nonempty original record")?;
    assert_index_fits(&fixture, below)?;
    let mut options = support::options();
    options.limits.record_bytes = bound;
    let exact = Fixture::with(
        [support::material(MTL, source.as_bytes(), Storage::Plain)],
        options.clone(),
        MaterialSourceLimits::default(),
    )?;
    assert_index_fits(&exact, below)?;
    assert_eq!(
        fs::read(exact.base.join("material.jsonl"))?,
        original,
        "additive origin transport does not consume the admitted payload bound or alter its bytes"
    );
    options.limits.record_bytes = below;
    let failure = Fixture::with(
        [support::material(MTL, source.as_bytes(), Storage::Plain)],
        options,
        MaterialSourceLimits::default(),
    )
    .err()
    .ok_or("record one byte above its bound was admitted")?;
    assert!(
        super::converted_artifact_support::contains(
            failure.as_ref(),
            |cause| matches!(cause, p4kx_media::MediaError::Limit { budget, limit }
                if *budget == "record_bytes" && *limit == below)
        ),
        "one byte below the exact record size fails specifically at record_bytes: {failure}"
    );
    Ok(())
}

fn assert_index_fits(fixture: &Fixture, smaller_bound: u64) -> TestResult {
    let bytes = fs::metadata(fixture.output.join("media-index.json"))?.len();
    assert!(
        bytes <= smaller_bound,
        "the complete pretty index ({bytes} bytes) fits both the exact and one-byte-smaller record caps ({smaller_bound})"
    );
    Ok(())
}
#[test]
fn completely_failed_required_shader_read_preserves_the_previous_complete_material_generation()
-> TestResult {
    let fixture = Fixture::new([support::material(MTL, b"<Material/>", Storage::Plain)])?;
    let previous = fs::read(fixture.output.join("media-index.json"))?;
    let source = fixture.folder.path().join("broken.p4k");
    ArchiveBuilder::new()
        .entry(support::material(
            MTL,
            b"<Material Shader='demo' StringGenMask='%A'/>",
            Storage::Plain,
        ))
        .entry(
            TestEntry::zstd(support::SHADER, support::DEFINITION)
                .encrypted()
                .with_payload([0_u8]),
        )
        .write_to(&source)?;
    let archive = Archive::open_with(
        source,
        OpenOptions::new().key(Some(p4kx_testkit::test_key())),
    )?;
    let mut options = fixture.options.clone();
    options.conflict = ConflictPolicy::Overwrite;
    let result = p4kx_media::export_with(
        &archive,
        &fixture.output,
        &options,
        &support::registry(MaterialSourceLimits::default())?,
    );
    let failure = result
        .err()
        .ok_or("a wholly failed actual input read published a Material generation")?;
    assert!(
        p4kx_extraction::archive_error_in(&failure).is_some(),
        "the actual failed target decode retains its original Archive error"
    );
    assert_eq!(
        fs::read(fixture.output.join("media-index.json"))?,
        previous,
        "failed reads cannot publish provenance over the prior complete generation"
    );
    Ok(())
}

#[test]
fn bounded_property_targets_keep_only_their_own_original_ordinal_conversion() -> TestResult {
    let mut options = support::options();
    options.limits.reference_candidates = 2;
    let definition =
        b"Version(1)\nProperty { Name(%A); }\nProperty { Name(%A); }\nProperty { Name(%A); }";
    let fixture = Fixture::with(
        [
            support::material(
                MTL,
                b"<Material Shader='demo' StringGenMask='%A'/>",
                Storage::Plain,
            ),
            support::material(support::SHADER, definition, Storage::Encrypted),
        ],
        options,
        MaterialSourceLimits::default(),
    )?;
    let prefix = "/data/payload/shader_references/1/reference/resolution";
    assert_eq!(
        fixture
            .record(MTL)?
            .1
            .pointer(&format!("{prefix}/reason/type"))
            .and_then(Value::as_str),
        Some("candidate-limit")
    );
    let aggregate = property_uses(&fixture, prefix, "conversion", "declaration-index")?;
    assert_eq!(
        aggregate,
        ["1", "2", "3"],
        "the bounded resolution retains both candidates and the actually observed discarded ordinal"
    );
    for (position, expected) in [(0, "1"), (1, "2")] {
        let pointer = format!("{prefix}/known_candidates/{position}/target/subresource/indices/0");
        let actual = fixture
            .record(MTL)?
            .1
            .pointer(&pointer)
            .and_then(Value::as_u64)
            .ok_or("actual Property ordinal")?;
        assert_eq!(actual.to_string(), expected);
        assert_eq!(
            property_uses(&fixture, &pointer, "conversion", "declaration-index")?,
            [expected],
            "one target cannot inherit another retained or discarded target's ordinal conversion"
        );
        assert_eq!(
            property_uses(&fixture, &pointer, "selector", "Name")?,
            [expected],
            "the target retains its own positive Name selector without later matching predicates"
        );
    }
    Ok(())
}

fn property_uses(
    fixture: &Fixture,
    pointer: &str,
    role: &str,
    field: &str,
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut ordinals: Vec<_> = fixture
        .contributions(MTL, pointer)?
        .iter()
        .filter(|used| {
            used.get("role").and_then(Value::as_str) == Some(role)
                && used.pointer("/location/namespace").and_then(Value::as_str)
                    == Some("shader-property")
                && used.pointer("/location/field").and_then(Value::as_str) == Some(field)
        })
        .filter_map(|used| {
            used.pointer("/location/indices/0")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .collect();
    ordinals.sort();
    ordinals.dedup();
    Ok(ordinals)
}
