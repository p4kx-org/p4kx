// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::io::{self, Read, Write as _};

use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceDeclaration, SourceDocument, SourceReadOptions,
};
use p4kx_extraction::sound::wwise_path;
use p4kx_extraction::{
    Archive, ArchiveWatch, Build, Entry, InputScope, SourceEncryptionState, SourceLocation,
    SourceUseRole, SourceUses,
};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::{Value, json};

use crate::Limits;
use crate::audio_control_fields::AtlFamily;
use crate::audio_control_origin as origins;
use crate::audio_control_payload::{AtlPayload, AtlPosition};
use crate::audio_control_projection::declaration::ObservedDeclaration;
use crate::origin_capture::OriginCapture;
use crate::origin_input::MediaOriginInput;

#[path = "audio_control_witness_rules.rs"]
mod rules;

use self::rules::{Family, Rule};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn sampled_original_atl_families_keep_their_exact_authored_values_and_origins() -> Result<(), String>
{
    let mut phase = "open Archive and bind Build";
    let result = (|| {
        let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
        let build = archive
            .build()
            .ok_or("authoritative local Build manifest")?;
        phase = "check physical ATL declarations";
        verify(&archive, build)
    })();
    result.map_err(|_: Box<dyn std::error::Error>| format!("ATL sample failed during {phase}"))
}

fn verify(archive: &Archive, build: &Build) -> TestResult {
    let watch = ArchiveWatch::new(archive);
    let mut missing: BTreeSet<_> = rules::RULES.iter().map(|rule| rule.check).collect();
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "ATL authored sample: Build {} {} {} {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id(),
    )?;
    out.flush()?;

    let mut inspected = 0;
    for entry in archive
        .entries()
        .iter()
        .filter(|entry| wwise_path::is_atl_definition(entry.path()))
    {
        watch.poll()?;
        verify_entry(archive, entry, &mut missing)?;
        inspected += 1;
        if missing.is_empty() {
            break;
        }
    }
    writeln!(
        out,
        "ATL sample inspected {inspected} original root XML documents"
    )?;
    report_coverage(&mut out, &missing)?;
    watch.check()?;
    let directory = archive
        .path()
        .parent()
        .ok_or("installed Archive channel directory")?;
    require(
        Build::read_from(directory)?.as_ref() == Some(build),
        "Build unchanged throughout witness",
    )?;
    require(
        missing.is_empty(),
        "all six original ATL families exercised",
    )
}

fn report_coverage(out: &mut impl io::Write, missing: &BTreeSet<&str>) -> TestResult {
    for check in missing {
        writeln!(out, "ATL sample missing original declaration group {check}")?;
    }
    for family in Family::ALL {
        let count = rules::RULES
            .iter()
            .filter(|rule| rule.family == family && !missing.contains(rule.check))
            .map(|rule| rule.check)
            .collect::<BTreeSet<_>>()
            .len();
        writeln!(
            out,
            "ATL {} checked original declaration groups {count}",
            family.name()
        )?;
    }
    out.flush()?;
    Ok(())
}

fn verify_entry(
    archive: &Archive,
    entry: &Entry,
    missing: &mut BTreeSet<&'static str>,
) -> TestResult {
    let mut bytes = Vec::new();
    archive
        .open_entry_verified(entry)?
        .read_to_end(&mut bytes)?;
    let document = SourceDocument::parse(&bytes, SourceReadOptions::default())?;
    let root = document.root();
    if root.name() != "ATLConfig" {
        return Ok(());
    }
    require(!document.is_lossy(), "complete original ATL syntax")?;
    let uses = SourceUses::with_compact_coordinates();
    let input = MediaOriginInput::observe(&uses, archive, entry, Limits::default().record_bytes)?;
    let mut pending = vec![root];
    while let Some(element) = pending.pop() {
        let syntax = element.syntax()?;
        for declaration in syntax.declarations() {
            let declaration = declaration?;
            let Some(rule) = rules::RULES.iter().find(|rule| {
                rule.tag == element.name()
                    && rule.attribute == declaration.qualified_name()
                    && missing.contains(rule.check)
            }) else {
                continue;
            };
            if declaration.value().is_empty() {
                continue;
            }
            verify_declaration(rule, &declaration, &input, entry)?;
            missing.remove(rule.check);
        }
        pending.extend(element.elements());
    }
    Ok(())
}

fn verify_declaration(
    rule: &Rule,
    declaration: &SourceDeclaration<'_>,
    input: &MediaOriginInput,
    entry: &Entry,
) -> TestResult {
    let observed = ObservedDeclaration::read(declaration, AtlFamily::from_tag(rule.tag))?;
    let position = observed.original.position;
    let original = serde_json::to_value(AtlPayload::Declaration(observed.original))?;
    let typed = observed
        .typed
        .ok_or("typed ATL family at original declaration")?;
    let projected = serde_json::to_value(AtlPayload::Field(typed.record))?;
    require(
        original
            .pointer("/payload/qualified_name")
            .and_then(Value::as_str)
            == Some(rule.attribute)
            && original.pointer("/payload/value").and_then(Value::as_str)
                == Some(declaration.value())
            && original.pointer("/payload/lexeme") == Some(&json!(declaration.lexeme())),
        "raw ATL spelling and value retained",
    )?;
    require(
        projected.pointer("/payload/field").and_then(Value::as_str) == Some(rule.field)
            && projected.pointer("/payload/typed_value") == Some(&rule.typed(declaration.value())),
        "typed ATL scalar agrees with the original declaration",
    )?;
    let expected = coordinate(declaration.position())?;
    for role in [SourceUseRole::Payload, SourceUseRole::Conversion] {
        verify_origin(input, entry, position, &expected, role)?;
    }
    Ok(())
}

fn coordinate(position: &SourceAttributePosition) -> TestResult<SourceLocation> {
    let (namespace, indices) = match position {
        SourceAttributePosition::Text(range) => (
            "atl-xml",
            [u64::try_from(range.start)?, u64::try_from(range.end)?],
        ),
        SourceAttributePosition::CryXml {
            node_index,
            attribute_index,
        } => (
            "atl-cryxml-attribute",
            [u64::from(*node_index), u64::from(*attribute_index)],
        ),
        _ => return Err("unsupported original XML declaration coordinate".into()),
    };
    Ok(SourceLocation::Subresource {
        namespace: namespace.into(),
        indices: Vec::from(indices),
        field: "declaration".into(),
    })
}

fn verify_origin(
    input: &MediaOriginInput,
    entry: &Entry,
    position: AtlPosition,
    expected: &SourceLocation,
    role: SourceUseRole,
) -> TestResult {
    let mut capture = OriginCapture::with_limit(&input.owner, Limits::default().record_bytes);
    origins::with_site(position, "declaration", |site| {
        capture.add_original(input, InputScope::Payload, site, role)
    })?;
    let observed = capture.finish();
    let [source_use] = observed.uses.as_ref() else {
        return Err("one matching physical ATL source use".into());
    };
    let description = source_use.description();
    let proof = observed.field_origin();
    require(
        proof.source_set_complete()
            && proof.contains_encrypted_source() == Some(entry.is_encrypted())
            && proof.source_uses_refs() == std::slice::from_ref(description.id()),
        "immediate origin identifies exactly one original contribution",
    )?;
    let encryption = if entry.is_encrypted() {
        SourceEncryptionState::Encrypted
    } else {
        SourceEncryptionState::Unencrypted
    };
    require(
        description.location() == expected
            && description.scope() == InputScope::Payload
            && description.role() == role
            && description.effective_encryption() == encryption,
        "use binds the original declaration byte domain, role and encryption",
    )?;
    let inventory = input.owner.freeze()?;
    let physical = inventory
        .iter_sources()
        .find(|source| source.id() == description.source())
        .ok_or("exact observed physical ATL owner")?;
    let physical = serde_json::to_value(physical)?;
    require(
        physical.get("entryIndex") == Some(&json!(entry.index()))
            && physical.pointer("/path/value").and_then(Value::as_str) == Some(entry.path())
            && physical.get("ancestors") == Some(&json!([])),
        "original root ATL owner matches its exact physical Entry",
    )
}

fn require(condition: bool, message: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(message.into())
    }
}
