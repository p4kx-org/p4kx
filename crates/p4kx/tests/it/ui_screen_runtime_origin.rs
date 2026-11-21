// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_screens::StillFormat;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestCanvas, TestEntry, TestValue, TestWidget, screens_schema,
};
use serde_json::{Value, json};

use super::{CANVAS, TestResult, marker, render};

#[test]
fn screen_runtime_target_keeps_its_exact_encrypted_widget_source() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = dir.path().join("Data.p4k");
    let canvas = TestCanvas::new(0x82b6, "origin_panel", 16.0, 8.0)
        .raw()
        .widget(TestWidget::display("root"))
        .widget(
            TestWidget::new("BuildingBlocks_WidgetRuntimeImage", "display")
                .parent("root")
                .with("imageSource", TestValue::text("SyntheticTarget")),
        );
    let forge = screens_schema(DataForgeBuilder::new())
        .record(canvas.record())
        .build();
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", forge).encrypted())
        .write_to(&archive)?;
    let output = dir.path().join("screen.png");
    let report = render(&archive, CANVAS, &output, StillFormat::Png)?;
    let location = report.pointer("/content_omissions/0/widget_input/record_index");
    assert_eq!(
        report.pointer("/content_omissions/0/code"),
        Some(&json!("runtime_target")),
        "identify the omitted game render target"
    );
    assert_eq!(
        report.pointer("/content_omissions/0/source"),
        Some(&json!("SyntheticTarget")),
        "keep the literal runtime name"
    );
    let field = "/content_omissions/0/source";
    assert_eq!(
        marker(&report, field),
        Some(true),
        "the source value comes from encrypted DataForge"
    );
    let refs = report
        .get("fieldOrigins")
        .and_then(|rows| rows.get(field))
        .and_then(|origin| origin.get("sourceUsesRefs"))
        .and_then(Value::as_array)
        .ok_or("runtime diagnostic source-use refs")?;
    let original = report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("render source-use inventory")?;
    assert!(
        original.iter().any(|used| {
            used.get("id").is_some_and(|id| refs.contains(id))
                && used.pointer("/location/recordIndex") == location
                && used.pointer("/location/field") == Some(&json!("scene[1].imageSource"))
        }),
        "the reported name cites the original widget field"
    );
    Ok(())
}
