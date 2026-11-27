// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_dataforge::{
    Annotations, DataForge, FlatColumn, FlatColumnSpan, FlatFormat, FlatRowObservation, FlatValues,
    LocalizationInput, ObservedResolution, PositionedValue, Resolution, TextResolver, TreeBuilder,
    Value,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{TestResult, builder};

#[test]
fn observed_flat_rows_reuse_exact_cells_and_original_nested_array_selectors() -> TestResult {
    let data = builder("=Cab, \"A\"").build();
    let forge = DataForge::open(&data)?;
    let original = forge.records().next().ok_or("source Record")?;
    let record = TreeBuilder::new().build_positioned_partial(&original)?;
    for (format, expected) in [
        (FlatFormat::Csv, br#""'=Cab, ""A""""#.as_slice()),
        (FlatFormat::CsvRaw, br#""=Cab, ""A""""#.as_slice()),
        (FlatFormat::CsvStrict, br#""=Cab, ""A""""#.as_slice()),
        (FlatFormat::Ndjson, br#""=Cab, \"A\"""#.as_slice()),
    ] {
        let mut writer = FlatValues::new(format);
        let mut raw = Vec::new();
        writer.write_header(false, &mut raw)?;
        writer.write_annotated(record.tree(), Annotations::none(), &mut raw)?;
        let mut observed = Vec::new();
        writer.write_header(false, &mut observed)?;
        let header = observed.len();
        let mut fields = NestedRows::default();
        writer.write_positioned_observed(&record, Annotations::none(), &mut observed, |row| {
            fields.capture(&row)
        })?;
        assert_eq!(observed, raw, "identical {format:?} bytes");
        let body = observed
            .get(header..)
            .ok_or("observed rows after the header")?;
        assert_eq!(
            cell_bytes(body, fields.name.as_ref())?,
            expected,
            "{format:?} value span"
        );
        assert_eq!(
            cell_bytes(body, fields.mark.as_ref())?,
            b"2",
            "nested array payload"
        );
        let lines = body.split(|&byte| byte == b'\n').skip(1).count();
        assert_eq!(
            fields.rows,
            u64::try_from(lines)?,
            "one callback per emitted fixture row"
        );
    }
    Ok(())
}

#[derive(Default)]
struct NestedRows {
    rows: u64,
    name: Option<FlatColumnSpan>,
    mark: Option<FlatColumnSpan>,
}

impl NestedRows {
    fn capture(&mut self, row: &FlatRowObservation<'_, '_, '_, '_>) -> io::Result<()> {
        assert_eq!(row.row_index(), self.rows, "dense observed row ordinals");
        assert!(
            row.columns().windows(2).all(
                |pair| matches!(pair, [first, second] if first.bytes().end <= second.bytes().start)
            ),
            "columns follow the exact emitted order"
        );
        self.rows += 1;
        match (row.field_path(), row.array_indices()) {
            ("name", []) => self.name = span(row, FlatColumn::Value).copied(),
            ("wheels.marks", [1, 1]) => {
                check_nested_route(row.route())?;
                self.mark = span(row, FlatColumn::Value).copied();
            }
            _ => {}
        }
        Ok(())
    }
}

fn check_nested_route(route: &[PositionedValue<'_, '_>]) -> io::Result<()> {
    let [cars, car, marks, mark] = route else {
        return Err(io::Error::other(
            "expected every original container and element",
        ));
    };
    assert_eq!(cars.field_name(), Some("wheels"), "outer array");
    assert_eq!(marks.field_name(), Some("marks"), "inner array");
    for original in [cars, marks] {
        assert!(
            original.field_name_sources().is_some(),
            "actual field name evidence"
        );
        assert!(
            original.array_header_source().is_some(),
            "actual array header words"
        );
    }
    assert!(
        matches!(car.value(), Value::Struct(_)),
        "selected first array element"
    );
    assert!(
        matches!(mark.value(), Value::UInt8(2)),
        "selected second array element"
    );
    assert!(
        mark.value_sources().complete(),
        "exact original leaf storage"
    );
    Ok(())
}

#[test]
fn localized_flat_columns_reuse_the_exact_lookup_and_keep_decisions() -> TestResult {
    let bytes = locale_record();
    let forge = DataForge::open(&bytes)?;
    let original = forge.records().next().ok_or("source Record")?;
    let record = TreeBuilder::new().build_positioned_partial(&original)?;
    let texts = Texts(AtomicUsize::new(0));
    let annotations = Annotations::none().with_texts(Some(&texts));
    let mut writer = FlatValues::new(FlatFormat::Ndjson);
    let mut out = Vec::new();
    let mut selected_text = None;
    writer.write_positioned_observed(&record, annotations, &mut out, |row| {
        let localization = row
            .localization()
            .ok_or_else(|| io::Error::other("actual lookup"))?;
        match row.field_path() {
            "keep" => {
                assert_eq!(
                    localization.resolution(),
                    &Resolution::Keep,
                    "the actual Keep decision"
                );
                assert_eq!(
                    localization.input(),
                    LocalizationInput::Lookup(None),
                    "consulted absent key"
                );
                assert!(
                    span(&row, FlatColumn::Text).is_none(),
                    "Keep emits no NDJSON text"
                );
            }
            "label" => {
                assert_eq!(
                    localization.input(),
                    LocalizationInput::Lookup(Some("localized")),
                    "the winning original lookup"
                );
                selected_text = span(&row, FlatColumn::Text).copied();
            }
            _ => return Err(io::Error::other("unexpected fixture row")),
        }
        Ok(())
    })?;
    assert_eq!(
        texts.0.load(Ordering::Relaxed),
        2,
        "each lookup is consulted once"
    );
    assert_eq!(
        cell_bytes(&out, selected_text.as_ref())?,
        br#""localized""#,
        "original chosen text"
    );
    Ok(())
}

#[test]
fn ordinal_escapes_retain_the_actual_duplicate_name_and_clear_it_for_siblings() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Base").field("label", TestType::Int32))
        .structure(
            TestStruct::new("Item")
                .parent("Base")
                .field("label", TestType::Int32)
                .field("$literal", TestType::Int32),
        )
        .record(TestRecord::new(1, "Item", "Item.One", "items/one.xml"))
        .build();
    let forge = DataForge::open(&bytes)?;
    let original = forge.records().next().ok_or("source Record")?;
    let record = TreeBuilder::new().build_positioned_partial(&original)?;
    let mut paths = Vec::new();
    FlatValues::new(FlatFormat::Ndjson).write_positioned_observed(
        &record,
        Annotations::none(),
        &mut Vec::new(),
        |row| {
            if row.field_path() == "$1" {
                let [earlier] = row.field_path_aliases() else {
                    return Err(io::Error::other("missing earlier collision evidence"));
                };
                let [current] = row.route() else {
                    return Err(io::Error::other("missing selected field occurrence"));
                };
                assert_eq!(
                    earlier.field_name(),
                    current.field_name(),
                    "same authored spelling"
                );
                let first = earlier
                    .field_name_sources()
                    .ok_or_else(|| io::Error::other("first property"))?;
                let second = current
                    .field_name_sources()
                    .ok_or_else(|| io::Error::other("second property"))?;
                assert_ne!(
                    first.sources(),
                    second.sources(),
                    "different original declaration ordinals"
                );
            } else {
                assert!(
                    row.field_path_aliases().is_empty(),
                    "no alias from a sibling"
                );
            }
            paths.push(row.field_path().to_owned());
            Ok(())
        },
    )?;
    assert_eq!(paths, ["label", "$1", "$2"], "existing path names");
    Ok(())
}

fn span<'row>(
    row: &FlatRowObservation<'row, '_, '_, '_>,
    column: FlatColumn,
) -> Option<&'row FlatColumnSpan> {
    row.columns().iter().find(|span| span.column() == column)
}

fn cell_bytes<'a>(body: &'a [u8], span: Option<&FlatColumnSpan>) -> io::Result<&'a [u8]> {
    let interval = span
        .ok_or_else(|| io::Error::other("missing emitted column"))?
        .bytes();
    let start = usize::try_from(interval.start).map_err(io::Error::other)?;
    let end = usize::try_from(interval.end).map_err(io::Error::other)?;
    body.get(start..end)
        .ok_or_else(|| io::Error::other("invalid value interval"))
}

struct Texts(AtomicUsize);

impl TextResolver for Texts {
    fn resolve<'a>(&'a self, key: &str) -> Resolution<'a> {
        match key {
            "@label" => Resolution::Text(Cow::Borrowed("localized")),
            _ => Resolution::Keep,
        }
    }

    fn resolve_observed<'a>(&'a self, key: &str) -> ObservedResolution<'a> {
        self.0.fetch_add(1, Ordering::Relaxed);
        let raw = (key == "@label").then_some("localized");
        ObservedResolution::from_lookup(self.resolve(key), raw)
    }
}

fn locale_record() -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Item")
                .field("keep", TestType::Locale)
                .field("label", TestType::Locale),
        )
        .record(
            TestRecord::new(1, "Item", "Item.One", "items/one.xml")
                .value("keep", TestValue::text("@keep"))
                .value("label", TestValue::text("@label")),
        )
        .build()
}
