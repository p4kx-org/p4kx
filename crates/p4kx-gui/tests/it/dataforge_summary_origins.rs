// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::path::Path;

use egui::accesskit::Role;
use egui::{Event, Key};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_extraction::{
    DataForge, FieldOrigin, SourceLocation, SourceUseDescription, SourceUseRole,
};
use p4kx_gui::{
    ArchiveKey, Browser, DataForgeSummary, DataForgeSummaryProvenance, Language, Model,
    RecordTreeStatus, ui,
};
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, test_key};

use crate::waiting::{RunQuiet, wait_for_model};

type TestResult = Result<(), Box<dyn Error>>;
const STORED_PATH: &str = "records/selected.xml";
const NORMALIZED_PATH: &str = "records/selected";

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("SummaryItem"))
        .record(TestRecord::new(
            1,
            "SummaryItem",
            "SummaryItem.First",
            STORED_PATH,
        ))
}

fn unreadable_name() -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = database().build();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("first record")?;
    let span = forge
        .original_text_span(record.name()?)
        .ok_or("borrowed name")?;
    let first = bytes
        .get_mut(usize::try_from(span.start)?)
        .ok_or("first byte")?;
    *first = 0xff;
    Ok(bytes)
}

fn opened(path: &Path, key: Option<ArchiveKey>) -> Result<Harness<'static, Model>, Box<dyn Error>> {
    let mut model = Model::new(Language::BritishEnglish);
    model.set_archive_key(key);
    let mut harness = Harness::builder()
        .with_context_setup(p4kx_gui::theme::apply)
        .with_size([1600.0, 900.0])
        .build_ui_state(
            |ui, model: &mut Model| {
                let _requests = ui::show(ui, model);
            },
            model,
        );
    harness.state_mut().open(path.to_owned(), || {});
    wait_for_model(&mut harness, "open input", |model| {
        model.browser().is_some()
    })?;
    harness.run_quiet();
    harness
        .get_by_role_and_label(Role::Button, "DataForge")
        .click();
    wait_for_model(&mut harness, "DataForge summary", |model| {
        summary(model).is_some()
    })?;
    harness.run_quiet();
    Ok(harness)
}

fn summary(model: &Model) -> Option<&DataForgeSummary> {
    let tree = model.browser().and_then(Browser::records)?;
    let RecordTreeStatus::Ready(summary) = tree.status() else {
        return None;
    };
    Some(summary)
}

fn used(
    proof: &DataForgeSummaryProvenance,
    origin: &FieldOrigin,
    check: impl Fn(&SourceUseDescription) -> bool,
) -> bool {
    proof
        .inventory()
        .iter_uses()
        .any(|used| origin.source_uses_refs().contains(used.id()) && check(used))
}

fn assert_headers(proof: &DataForgeSummaryProvenance) {
    for (name, start, origin) in [
        ("version", 4, &proof.version),
        ("structCount", 16, &proof.struct_count),
        ("enumCount", 24, &proof.enum_count),
        ("recordCount", 32, &proof.record_count),
    ] {
        assert_eq!(origin.contains_encrypted_source(), Some(true), "{name}");
        assert!(
            used(proof, origin, |used| matches!(
                used.location(),
                SourceLocation::Subresource { namespace, indices, field }
                    if namespace == "dataforgeHeader" && field == name && *indices == [start, start + 4]
            )),
            "the original {name} word"
        );
    }
}

fn assert_first_record(proof: &DataForgeSummaryProvenance, bytes: &[u8]) -> TestResult {
    let [row] = proof.rows() else {
        return Err("one Record row".into());
    };
    assert!(
        used(proof, &row.name, |used| {
            used.role() == SourceUseRole::Conversion
                && matches!(
                    used.location(),
                    SourceLocation::DataForge { record_index: 0, field } if field == "guid"
                )
        }),
        "the name fallback derives from the original GUID"
    );
    assert!(
        !used(proof, &row.name, |used| used.role()
            == SourceUseRole::Payload),
        "undecodable name bytes do not contribute to the fallback"
    );
    assert!(
        row.name.source_set_complete(),
        "exact provenance of the fallback"
    );
    assert!(
        used(proof, &row.type_name, |used| matches!(
            used.location(),
            SourceLocation::Subresource { namespace, indices, field }
                if namespace == "dataforgeStructDefinition" && *indices == [0] && field == "name"
        )),
        "the type uses its struct definition"
    );
    let forge = DataForge::open(bytes)?;
    let record = forge.record(0).ok_or("original row")?;
    let span = forge
        .original_text_span(record.stored_file_name()?)
        .ok_or("filename bytes")?;
    assert!(
        used(proof, &row.record_path, |used| matches!(
            used.location(),
            SourceLocation::Subresource { namespace, indices, field }
                if namespace == "dataforgeStringBytes" && field == "fileName"
                    && *indices == [span.start, span.end]
                    && used.role() == SourceUseRole::Conversion
        )),
        "path normalisation uses the full original filename"
    );
    Ok(())
}

fn proof_on_hover(harness: &mut Harness<'_, Model>, label: &str) -> TestResult {
    harness.event(Event::PointerGone);
    harness.run_steps(2);
    let panel = ui::regions(&harness.ctx).preview.ok_or("preview region")?;
    let point = harness
        .query_all_by_label(label)
        .find(|node| panel.contains(node.rect().center()))
        .ok_or_else(|| format!("DataForge summary label {label:?} must be visible"))?
        .rect()
        .center();
    harness.event(Event::PointerMoved(point));
    harness.run_steps(4);
    assert!(
        harness
            .query_by_label("Includes encrypted source data")
            .is_some(),
        "{label}"
    );
    Ok(())
}

#[test]
fn dataforge_summary_marks_header_facts_and_rows_from_the_admitted_entry() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let encrypted = unreadable_name()?;
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game.dcb", database().build()))
        .entry(TestEntry::deflated("Data\\Game2.dcb", &encrypted[..]).encrypted())
        .write_to(&path)?;
    let mut harness = opened(&path, Some(test_key()))?;
    let summary = summary(harness.state()).ok_or("summary")?;
    let proof = summary.provenance()?;
    assert_eq!(
        proof.inventory().source_count(),
        1,
        "only the admitted encrypted DCB"
    );
    assert_headers(proof);
    assert_first_record(proof, &encrypted)?;
    let [row] = summary.first_records() else {
        return Err("first row".into());
    };
    assert_eq!(
        row.name(),
        row.guid().to_string(),
        "unchanged GUID fallback"
    );
    assert_eq!(
        row.record_path(),
        NORMALIZED_PATH,
        "unchanged displayed path"
    );

    harness.ctx.style_mut_of(harness.ctx.theme(), |style| {
        style.interaction.tooltip_delay = 0.0;
        style.interaction.show_tooltips_only_when_still = false;
    });
    proof_on_hover(&mut harness, "Version")?;
    harness
        .get_by_role_and_label(Role::ScrollView, "DataForge summary")
        .focus();
    harness.run_quiet();
    harness.key_press(Key::End);
    harness.run_quiet();
    proof_on_hover(&mut harness, NORMALIZED_PATH)?;
    Ok(())
}

#[test]
fn a_standalone_dataforge_summary_keeps_unknown_original_encryption() -> TestResult {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Game2.dcb");
    std::fs::write(&path, database().build())?;
    let harness = opened(&path, None)?;
    let proof = summary(harness.state())
        .ok_or("standalone summary")?
        .provenance()?;
    assert!(
        proof.version.source_set_complete(),
        "known standalone physical source"
    );
    assert_eq!(
        proof.version.contains_encrypted_source(),
        None,
        "unknown P4K lineage"
    );
    let [row] = proof.rows() else {
        return Err("one Record row".into());
    };
    assert!(
        row.name.source_set_complete(),
        "known standalone string bytes"
    );
    assert_eq!(
        row.name.contains_encrypted_source(),
        None,
        "unknown P4K lineage of the name"
    );
    Ok(())
}
