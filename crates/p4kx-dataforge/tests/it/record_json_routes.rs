// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::error::Error;
use std::io;

use p4kx_dataforge::{
    DataForge, JsonLayout, PositionedRecord, PositionedSources, PositionedValue, RecordJson,
    RecordJsonBytes, RecordJsonObserver, RecordJsonSource, RecordJsonStep, TreeBuilder,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type Evidence = Vec<RecordJsonSource<'static>>;

const BASE_VALUE: usize = 0;
const DERIVED_VALUE: usize = 1;
const POINTER_ARRAY: usize = 3;
const CLASS_ARRAY: usize = 4;
const DIRECT_POINTER: usize = 5;
const FRAME: usize = 6;
const OUTSIDE: usize = 8;
const LEAF_TYPE: u32 = 1;
const FRAME_TYPE: u32 = 2;

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("LeafBase").field("same", TestType::Int32))
        .structure(
            TestStruct::new("Leaf")
                .parent("LeafBase")
                .field("same", TestType::Int32)
                .field("other", TestType::Int32),
        )
        .structure(
            TestStruct::new("Frame")
                .array("$routes", TestType::StrongPointer("Leaf".to_owned()))
                .array("classes", TestType::Class("Leaf".to_owned()))
                .field("direct", TestType::StrongPointer("Leaf".to_owned())),
        )
        .structure(TestStruct::new("RootBase").field("frame", TestType::Class("Frame".to_owned())))
        .structure(
            TestStruct::new("Root")
                .parent("RootBase")
                .field("head", TestType::Int32)
                .field("outside", TestType::Class("Leaf".to_owned())),
        )
        .instance("target", leaf(17))
        .instance("direct-target", leaf(29))
        .record(
            TestRecord::new(42, "Root", "Root.One", "one.xml")
                .value("outside", TestValue::Struct(leaf(31)))
                .value(
                    "frame",
                    TestValue::Struct(
                        TestInstance::new("Frame")
                            .value(
                                "$routes",
                                TestValue::Array(vec![
                                    TestValue::Null,
                                    TestValue::pointer("target"),
                                ]),
                            )
                            .value(
                                "classes",
                                TestValue::Array(vec![
                                    TestValue::Struct(leaf(11)),
                                    TestValue::Struct(leaf(19)),
                                ]),
                            )
                            .value("direct", TestValue::pointer("direct-target")),
                    ),
                ),
        )
        .build()
}

fn leaf(value: i32) -> TestInstance {
    TestInstance::new("Leaf")
        .value("same", TestValue::Int32(value))
        .value("other", TestValue::Int32(23))
}

struct Expected<'path>(BTreeMap<&'path str, Evidence>);

impl RecordJsonObserver for Expected<'_> {
    fn value(
        &mut self,
        pointer: &str,
        sources: &[RecordJsonSource<'_>],
        complete: bool,
    ) -> io::Result<()> {
        if let Some(expected) = self.0.remove(pointer) {
            assert!(complete, "all actual source coordinates for {pointer}");
            assert!(
                sources.iter().all(|source| expected.contains(source)),
                "no extra coordinates for {pointer}: {sources:?}"
            );
            assert!(
                expected.iter().all(|source| sources.contains(source)),
                "all required coordinates for {pointer}: {sources:?}"
            );
        }
        Ok(())
    }
}

fn position<'source>(forge: &'source DataForge<'source>) -> TestResult<PositionedRecord<'source>> {
    let record = forge.record(0).ok_or("original root Record")?;
    Ok(TreeBuilder::new().build_positioned(&record)?)
}

fn observed(
    input: &PositionedRecord<'_>,
    steps: &[RecordJsonStep],
    expected: BTreeMap<&str, Evidence>,
) -> TestResult<Value> {
    let mut output = Vec::new();
    let mut expected = Expected(expected);
    RecordJson::new(JsonLayout::Compact).write_observed_value(
        input,
        steps,
        &mut output,
        &mut expected,
    )?;
    assert!(
        expected.0.is_empty(),
        "every required JSON Pointer was emitted: {:?}",
        expected.0
    );
    Ok(serde_json::from_slice(&output)?)
}

const fn storage(index: usize, target_used: bool) -> RecordJsonSource<'static> {
    RecordJsonSource::PropertyStorage { index, target_used }
}

fn original_span(
    position: &PositionedSources,
    kind: RecordJsonBytes,
    use_as: RecordJsonBytes,
) -> TestResult<RecordJsonSource<'static>> {
    position
        .sources()
        .iter()
        .find_map(|&source| {
            if let RecordJsonSource::Bytes {
                start,
                end,
                kind: stored,
            } = source
                && stored == kind
            {
                Some(RecordJsonSource::Bytes {
                    start,
                    end,
                    kind: use_as,
                })
            } else {
                None
            }
        })
        .ok_or_else(|| format!("original {kind:?} bytes").into())
}

fn pointer_path<'record, 'source>(
    frame: PositionedValue<'record, 'source>,
) -> TestResult<(PositionedValue<'record, 'source>, Evidence)> {
    let fields = frame.fields()?.ok_or("inline Frame")?;
    let array = fields.get(0).ok_or("inherited Frame pointer array")?;
    let pointer = array.element(1).ok_or("actual pointer pool slot")?;
    let evidence = vec![
        storage(FRAME, true),
        storage(POINTER_ARRAY, false),
        array
            .array_header_source()
            .ok_or("original pointer array header")?,
        original_span(
            &pointer.storage_sources(),
            RecordJsonBytes::ArrayElement,
            RecordJsonBytes::PointerSelector,
        )?,
    ];
    Ok((pointer, evidence))
}

fn class_path<'record, 'source>(
    frame: PositionedValue<'record, 'source>,
) -> TestResult<(PositionedValue<'record, 'source>, Evidence)> {
    let fields = frame.fields()?.ok_or("inline Frame")?;
    let array = fields.get(1).ok_or("Frame class array")?;
    let member = array.element(1).ok_or("actual inline class member")?;
    Ok((
        member,
        vec![
            storage(FRAME, true),
            storage(CLASS_ARRAY, true),
            array
                .array_header_source()
                .ok_or("original class array header")?,
        ],
    ))
}

fn leaf_value(
    route: &[RecordJsonSource<'static>],
    leaf: PositionedValue<'_, '_>,
    ordinal: usize,
    property: usize,
) -> TestResult<Evidence> {
    let fields = leaf.fields()?.ok_or("owned Leaf")?;
    let field = fields
        .get(ordinal)
        .ok_or("original inherited-first leaf slot")?;
    let mut evidence = route.to_vec();
    evidence.push(storage(property, false));
    evidence.push(original_span(
        &field.storage_sources(),
        RecordJsonBytes::Slot,
        RecordJsonBytes::Slot,
    )?);
    Ok(evidence)
}

#[test]
fn selected_record_value_uses_only_original_route_and_leaf_storage() -> TestResult {
    use RecordJsonStep::{Element, Field};

    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let input = position(&forge)?;
    let frame = input.root_fields()?.get(0).ok_or("inherited frame field")?;
    let (pointer, through_pointer) = pointer_path(frame)?;
    let (class, through_class) = class_path(frame)?;

    let inherited = leaf_value(&through_pointer, pointer, 0, BASE_VALUE)?;
    let duplicate = leaf_value(&through_pointer, pointer, 1, DERIVED_VALUE)?;
    assert_ne!(
        inherited, duplicate,
        "equal field names do not conflate their original storage"
    );

    let field = observed(
        &input,
        &[Field(0), Field(0), Element(1), Field(1)],
        BTreeMap::from([("", duplicate)]),
    )?;
    assert_eq!(
        field,
        json!(0),
        "the second physical field owns its default value"
    );
    let field = observed(
        &input,
        &[Field(0), Field(1), Element(1), Field(0)],
        BTreeMap::from([("", leaf_value(&through_class, class, 0, BASE_VALUE)?)]),
    )?;
    assert_eq!(
        field,
        json!(19),
        "the selected inline class supplies its own leaf"
    );
    Ok(())
}

#[test]
fn nested_record_json_keeps_wrapper_and_type_proofs_at_their_actual_source() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let input = position(&forge)?;
    let frame = input.root_fields()?.get(0).ok_or("inherited frame field")?;
    let expected = nested_evidence(frame)?;
    let selected = observed(&input, &[RecordJsonStep::Field(0)], expected)?;
    assert_eq!(
        selected.pointer("/$0/name"),
        Some(&json!("$routes")),
        "the reserved array name is represented in its ordinal wrapper"
    );
    assert_eq!(
        selected.pointer("/direct/$type"),
        Some(&json!("Leaf")),
        "the direct pointer owns a distinct Leaf instance"
    );
    assert_eq!(
        selected.pointer("/direct/same"),
        Some(&json!(29)),
        "the directly owned instance retains its separately authored value"
    );
    let mut full = Vec::new();
    RecordJson::new(JsonLayout::Compact).write(input.tree(), &mut full)?;
    let full: Value = serde_json::from_slice(&full)?;
    assert_eq!(
        Some(&selected),
        full.get("frame"),
        "subtree and canonical Record wire values agree"
    );
    Ok(())
}

fn nested_evidence(frame: PositionedValue<'_, '_>) -> TestResult<BTreeMap<&'static str, Evidence>> {
    let (pointer, through_pointer) = pointer_path(frame)?;
    let (class, through_class) = class_path(frame)?;
    let fields = frame.fields()?.ok_or("inline Frame")?;
    let mut name = vec![storage(FRAME, true)];
    let source = fields
        .get(0)
        .and_then(|site| site.field_name_sources())
        .ok_or("wrapped field label")?;
    name.extend_from_slice(source.sources());
    let direct = fields.get(2).ok_or("direct pointer field")?;
    let direct_range = original_span(
        &direct.storage_sources(),
        RecordJsonBytes::Slot,
        RecordJsonBytes::PointerSelector,
    )?;
    let with_type = |mut proof: Evidence| {
        proof.push(RecordJsonSource::StructName { index: LEAF_TYPE });
        proof
    };
    Ok(BTreeMap::from([
        (
            "/$type",
            vec![
                storage(FRAME, true),
                RecordJsonSource::StructName { index: FRAME_TYPE },
            ],
        ),
        ("/$0/name", name),
        ("/$0/value/1/$type", with_type(through_pointer.clone())),
        (
            "/$0/value/1/same",
            leaf_value(&through_pointer, pointer, 0, BASE_VALUE)?,
        ),
        (
            "/$0/value/1/$1/value",
            leaf_value(&through_pointer, pointer, 1, DERIVED_VALUE)?,
        ),
        ("/classes/1/$type", with_type(through_class.clone())),
        (
            "/classes/1/same",
            leaf_value(&through_class, class, 0, BASE_VALUE)?,
        ),
        (
            "/direct/$type",
            with_type(vec![
                storage(FRAME, true),
                storage(DIRECT_POINTER, false),
                direct_range,
            ]),
        ),
    ]))
}

#[test]
fn whole_record_json_keeps_nested_routes_and_releases_sibling_ancestors() -> TestResult {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let input = position(&forge)?;
    let root = input.root_fields()?;
    let frame = root.get(0).ok_or("inherited Frame")?;
    let outside = root.get(2).ok_or("separate root child")?;
    let mut nested = nested_evidence(frame)?;
    let mut fields = BTreeMap::new();
    for (full, relative) in [
        ("/frame/$0/name", "/$0/name"),
        ("/frame/$0/value/1/same", "/$0/value/1/same"),
        ("/frame/classes/1/$type", "/classes/1/$type"),
        ("/frame/direct/$type", "/direct/$type"),
    ] {
        fields.insert(
            full,
            nested
                .remove(relative)
                .ok_or("independent subtree expectation")?,
        );
    }
    fields.insert(
        "/outside/same",
        leaf_value(&[storage(OUTSIDE, true)], outside, 0, BASE_VALUE)?,
    );
    let mut expected = Expected(fields);
    let mut output = Vec::new();
    let mut writer = RecordJson::new(JsonLayout::Compact);
    writer.write_observed(&input, &mut output, &mut expected)?;
    assert!(
        expected.0.is_empty(),
        "each full-Record route was observed: {:?}",
        expected.0
    );

    let mut canonical = Vec::new();
    writer.write(input.tree(), &mut canonical)?;
    assert_eq!(
        output, canonical,
        "full observed and unobserved JSON bytes match"
    );
    Ok(())
}
