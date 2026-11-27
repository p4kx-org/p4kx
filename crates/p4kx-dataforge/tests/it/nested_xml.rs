// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Record XML of nested structs, arrays, References and nulls.

use std::error::Error;
use std::{io, ptr};

use p4kx_dataforge::{
    Annotations, DataForge, Field, RecordTree, RecordXml, RecordXmlHeaderMember, RecordXmlSite,
    RecordXmlSpan, RecordXmlValueMember, Value,
};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

const SAMPLE: u128 = 0xcbee_34fd_6441_4000_b99f_924f_7f26_e19f;
const MAKER: u128 = 0x50e8_a9f6_f956_4000_8ace_7c0c_a9a9_1f5d;
const MISSING: u128 = 0x0000_0001_0002_0003_0004_0000_0000_0005;

fn wheel(size: i32) -> TestValue {
    TestValue::Struct(TestInstance::new("Wheel").value("size", TestValue::Int32(size)))
}

fn car() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Vector")
                .field("x", TestType::Single)
                .field("y", TestType::Single),
        )
        .structure(TestStruct::new("Wheel").field("size", TestType::Int32))
        .structure(TestStruct::new("Maker"))
        .structure(
            TestStruct::new("Car")
                .field("name", TestType::String)
                .field("position", TestType::Class("Vector".to_owned()))
                .array("wheels", TestType::Class("Wheel".to_owned()))
                .field("spare", TestType::StrongPointer("Wheel".to_owned()))
                .field("missing", TestType::StrongPointer("Wheel".to_owned()))
                .array("tags", TestType::String)
                .field("maker", TestType::Reference)
                .field("front", TestType::WeakPointer("Wheel".to_owned()))
                .array("bad:name", TestType::Int32)
                .field("lost", TestType::Reference),
        )
        .instance(
            "spare",
            TestInstance::new("Wheel").value("size", TestValue::Int32(15)),
        )
        .record(
            TestRecord::new(
                SAMPLE,
                "Car",
                "Car.One",
                "libs/foundry/records/cars/one.xml",
            )
            .value("name", TestValue::text("Avenger"))
            .value(
                "position",
                TestValue::Struct(
                    TestInstance::new("Vector")
                        .value("x", TestValue::Single(1.0))
                        .value("y", TestValue::Single(2.0)),
                ),
            )
            .value("wheels", TestValue::Array(vec![wheel(16), wheel(17)]))
            .value("spare", TestValue::pointer("spare"))
            .value(
                "tags",
                TestValue::Array(vec![TestValue::text("a"), TestValue::text("b")]),
            )
            .value("maker", TestValue::Reference(MAKER))
            .value("front", TestValue::pointer("spare"))
            .value("lost", TestValue::Reference(MISSING)),
        )
        .record(TestRecord::new(
            MAKER,
            "Maker",
            "Maker.Aegis",
            "libs/foundry/records/makers/aegis.xml",
        ))
}

#[derive(Clone, Copy)]
enum Mode {
    Document,
    Element(usize),
    Wrapped(usize),
}

/// Returns the XML of the first Record, checking that versions 6 and 8 give the same bytes.
fn first_xml(builder: &DataForgeBuilder, mode: Mode) -> Result<String, Box<dyn Error>> {
    let mut texts = Vec::new();
    for version in [6, 8] {
        let bytes = builder.clone().version(version).build();
        let forge = DataForge::open(&bytes)?;
        let tree = forge.records().next().ok_or("no Record")?.tree()?;
        let mut out = Vec::new();
        let mut xml = RecordXml::new();
        match mode {
            Mode::Document => xml.write(&tree, &mut out)?,
            Mode::Element(depth) => xml.write_element(&tree, depth, &mut out)?,
            Mode::Wrapped(depth) => xml.write_wrapped_element(&tree, depth, &mut out)?,
        }
        texts.push(String::from_utf8(out)?);
    }
    let [six, eight] = <[String; 2]>::try_from(texts).map_err(|_| "two versions")?;
    assert_eq!(six, eight, "versions 6 and 8 give the same XML");
    Ok(six)
}

#[test]
fn nested_values_are_child_elements_in_declaration_order() -> TestResult {
    let xml = first_xml(&car(), Mode::Document)?;

    assert_eq!(
        xml,
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<Car xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" path=\"libs/foundry/records/cars/one\" p4kx:name=\"Car.One\" name=\"Avenger\">\n",
            "  <position>\n",
            "    <Vector x=\"1\" y=\"2\"/>\n",
            "  </position>\n",
            "  <wheels>\n",
            "    <Wheel size=\"16\"/>\n",
            "    <Wheel size=\"17\"/>\n",
            "  </wheels>\n",
            "  <spare>\n",
            "    <Wheel size=\"15\"/>\n",
            "  </spare>\n",
            "  <missing>\n",
            "    <p4kx:null/>\n",
            "  </missing>\n",
            "  <tags>\n",
            "    <p4kx:value value=\"a\"/>\n",
            "    <p4kx:value value=\"b\"/>\n",
            "  </tags>\n",
            "  <maker>\n",
            "    <p4kx:reference guid=\"50e8a9f6-f956-4000-8ace-7c0ca9a91f5d\" path=\"libs/foundry/records/makers/aegis\" type=\"Maker\"/>\n",
            "  </maker>\n",
            "  <front>\n",
            "    <p4kx:reference guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" path=\"libs/foundry/records/cars/one\" type=\"Wheel\" at=\"spare\"/>\n",
            "  </front>\n",
            "  <p4kx:field name=\"bad:name\"/>\n",
            "  <p4kx:error field=\"lost\" reason=\"the Reference refers to the Record 00000001-0002-0003-0004-000000000005, which does not exist\" guid=\"00000001-0002-0003-0004-000000000005\"/>\n",
            "</Car>\n",
        ),
        "the full shape"
    );
    let parsed = roxmltree::Document::parse(&xml)?;
    assert_eq!(
        parsed
            .root_element()
            .children()
            .filter(roxmltree::Node::is_element)
            .count(),
        9,
        "one child element per field that is no attribute"
    );
    Ok(())
}

#[test]
fn a_struct_type_that_is_no_plain_name_and_errors_in_arrays_are_written() -> TestResult {
    let builder = DataForgeBuilder::new()
        .structure(
            TestStruct::new("Odd Type")
                .field("type", TestType::Int8)
                .field("a", TestType::Int8),
        )
        .structure(
            TestStruct::new("Holder")
                .field("inner", TestType::Class("Odd Type".to_owned()))
                .array("names", TestType::String),
        )
        .record(
            TestRecord::new(SAMPLE, "Holder", "Holder.One", "holder.xml")
                .value(
                    "inner",
                    TestValue::Struct(
                        TestInstance::new("Odd Type")
                            .value("type", TestValue::Int8(3))
                            .value("a", TestValue::Int8(1)),
                    ),
                )
                .value(
                    "names",
                    TestValue::Array(vec![TestValue::Raw(vec![0xff; 4]), TestValue::text("ok")]),
                ),
        );

    let xml = first_xml(&builder, Mode::Element(1))?;

    assert_eq!(
        xml,
        concat!(
            "  <Holder xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\" guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" path=\"holder\" p4kx:name=\"Holder.One\">\n",
            "    <inner>\n",
            "      <p4kx:struct type=\"Odd Type\" a=\"1\">\n",
            "        <p4kx:field name=\"type\" value=\"3\"/>\n",
            "      </p4kx:struct>\n",
            "    </inner>\n",
            "    <names>\n",
            "      <p4kx:error reason=\"offset 4294967295 in the text table is not a valid string\"/>\n",
            "      <p4kx:value value=\"ok\"/>\n",
            "    </names>\n",
            "  </Holder>\n",
        ),
        "the root element alone, indented one level"
    );
    Ok(())
}

#[test]
fn nested_records_give_byte_identical_xml_every_time() -> TestResult {
    assert_eq!(
        first_xml(&car(), Mode::Document)?,
        first_xml(&car(), Mode::Document)?,
        "deterministic"
    );
    Ok(())
}

#[test]
fn a_wrapped_element_leaves_the_namespace_to_its_wrapper() -> TestResult {
    let xml = first_xml(&car(), Mode::Wrapped(1))?;

    assert!(
        xml.starts_with("  <Car guid=\"cbee34fd-6441-4000-b99f-924f7f26e19f\" path=\"libs/foundry/records/cars/one\" p4kx:name=\"Car.One\" name=\"Avenger\">\n"),
        "no xmlns:p4kx: {xml}"
    );
    let wrapped = format!(
        "<records xmlns:p4kx=\"https://github.com/p4kx-org/p4kx/record-xml\">\n{xml}</records>\n"
    );
    roxmltree::Document::parse(&wrapped)?;
    Ok(())
}

#[test]
fn observed_xml_is_byte_identical_and_addresses_exact_values() -> TestResult {
    let bytes = car().build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.records().next().ok_or("Car Record")?.tree()?;
    let mut writer = RecordXml::new();
    let mut plain = Vec::new();
    let mut observed = Vec::new();
    let mut spans = Vec::new();
    writer.write(&tree, &mut plain)?;
    writer.write_observed(&tree, Annotations::none(), &mut observed, &mut |span| {
        spans.push(span);
        Ok(())
    })?;
    assert_eq!(observed, plain, "observation preserves every XML byte");
    assert!(
        spans.windows(2).all(|pair| {
            pair.first()
                .zip(pair.last())
                .is_some_and(|(left, right)| left.bytes.end <= right.bytes.start)
        }),
        "fragments are in nonoverlapping output order"
    );
    assert_eq!(
        fragments(&observed, &spans, |site| matches!(
            site,
            RecordXmlSite::Header(RecordXmlHeaderMember::Type)
        )),
        ["Car", "Car"],
        "both root tag names"
    );
    assert_array_origins(&tree, &observed, &spans)?;
    assert_value_origins(&tree, &observed, &spans)
}

fn field<'record, 'source>(
    tree: &'record RecordTree<'source>,
    name: &str,
) -> Result<&'record Value<'source>, Box<dyn Error>> {
    tree.fields()
        .iter()
        .find(|field| field.name() == name)
        .map(Field::value)
        .ok_or_else(|| format!("missing field {name}").into())
}

fn assert_array_origins(
    tree: &RecordTree<'_>,
    xml: &[u8],
    spans: &[RecordXmlSpan<'_, '_>],
) -> TestResult {
    let owner = field(tree, "tags")?;
    let Value::Array(tags) = owner else {
        return Err("tags array".into());
    };
    let first_tag = tags.first().ok_or("first array value")?;
    let second_tag = tags.get(1).ok_or("second array value")?;
    assert_eq!(
        value_fragments(xml, spans, first_tag, RecordXmlValueMember::Value),
        ["a"],
        "first array occurrence"
    );
    assert_eq!(
        value_fragments(xml, spans, second_tag, RecordXmlValueMember::Value),
        ["b"],
        "second array occurrence"
    );
    assert_eq!(
        value_fragments(xml, spans, owner, RecordXmlValueMember::Name),
        ["tags", "tags"],
        "start and end names refer to the same owning field"
    );
    Ok(())
}

fn assert_value_origins(
    tree: &RecordTree<'_>,
    xml: &[u8],
    spans: &[RecordXmlSpan<'_, '_>],
) -> TestResult {
    use RecordXmlValueMember::{Guid, Path, Value};

    assert_eq!(
        value_fragments(xml, spans, field(tree, "missing")?, Value),
        ["p4kx:null"],
        "null derived from the stored pointer"
    );
    assert_eq!(
        value_fragments(xml, spans, field(tree, "maker")?, Path),
        ["libs/foundry/records/makers/aegis"],
        "selected Reference target path"
    );
    assert_eq!(
        value_fragments(xml, spans, field(tree, "lost")?, Guid),
        ["00000001-0002-0003-0004-000000000005"],
        "unresolved stored GUID"
    );
    Ok(())
}

fn value_fragments<'a>(
    xml: &'a [u8],
    spans: &[RecordXmlSpan<'_, '_>],
    datum: &Value<'_>,
    member: RecordXmlValueMember,
) -> Vec<&'a str> {
    fragments(
        xml,
        spans,
        |site| matches!(site, RecordXmlSite::Value { value, member: part, .. } if ptr::eq(value, datum) && part == member),
    )
}

fn fragments<'a>(
    bytes: &'a [u8],
    spans: &[RecordXmlSpan<'_, '_>],
    matches: impl Fn(RecordXmlSite<'_, '_>) -> bool,
) -> Vec<&'a str> {
    spans
        .iter()
        .filter(|span| matches(span.site))
        .filter_map(|span| {
            let start = usize::try_from(span.bytes.start).ok()?;
            let end = usize::try_from(span.bytes.end).ok()?;
            std::str::from_utf8(bytes.get(start..end)?).ok()
        })
        .collect()
}

#[test]
fn an_xml_observer_can_stop_the_writer_at_an_exact_fragment() -> TestResult {
    let bytes = car().build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge.records().next().ok_or("Car Record")?.tree()?;
    let mut out = Vec::new();
    let mut observed = 0;
    let result = RecordXml::new().write_observed(&tree, Annotations::none(), &mut out, &mut |_| {
        observed += 1;
        Err(io::Error::from(io::ErrorKind::Interrupted))
    });
    assert!(
        matches!(result, Err(error) if error.kind() == io::ErrorKind::Interrupted),
        "observer failure reaches the original caller"
    );
    assert_eq!(observed, 1, "the first observer failure stops serializing");
    assert!(
        out.ends_with(b"<Car"),
        "the reported range was fully written"
    );
    Ok(())
}
