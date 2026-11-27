// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::port;
use crate::{
    Attachment, LoadoutResolver, PortDefinition, PortDefinitionPosition, PortHelperChoice,
    PortHelperProblem, PortHelperTagMatch, PortNode, PortSource, PortTransform, PortVisibility,
};

const PORT: &str = "SItemPortDef";
const BONE: &str = "SItemPortDefAttachmentImplementationBone";
const HELPER_NODE: &str = "SItemPortDefHelperNode";
const HELPER: &str = "SItemPortDefHelper";

fn helper(name: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HELPER)
            .value("Name", TestValue::text(name))
            .value("ItemOffsetHelperName", TestValue::text("item_origin"))
            .value(
                "Offset",
                TestValue::Struct(TestInstance::new("Offset").value(
                    "Position",
                    TestValue::Struct(
                        TestInstance::new("Vector").value("z", TestValue::Single(0.25)),
                    ),
                )),
            ),
    )
}

fn node(tags: &str, helper: TestValue, children: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new(HELPER_NODE)
            .value("Tags", TestValue::text(tags))
            .value("Helper", helper)
            .value("SubHelpers", TestValue::Array(children)),
    )
}

fn nested_helpers(other_selector: &str) -> TestValue {
    node(
        "",
        helper("spine"),
        vec![
            TestValue::Null,
            node(
                "Heavy Variant",
                TestValue::Null,
                vec![node("Large", helper("spine_low"), Vec::new())],
            ),
            node(other_selector, helper("spine_high"), Vec::new()),
        ],
    )
}

fn port_fixture(root: TestValue, tag_type: TestType) -> Vec<u8> {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new(PORT).field("AttachmentImplementation", TestType::Class(BONE.into())),
        )
        .structure(TestStruct::new(BONE).field("Helper", TestType::Class(HELPER_NODE.into())))
        .structure(
            TestStruct::new(HELPER_NODE)
                .field("Tags", tag_type)
                .field("Helper", TestType::StrongPointer(HELPER.into()))
                .array("SubHelpers", TestType::StrongPointer(HELPER_NODE.into())),
        )
        .structure(
            TestStruct::new(HELPER)
                .field("Name", TestType::String)
                .field("ItemOffsetHelperName", TestType::String)
                .field("Offset", TestType::Class("Offset".into())),
        )
        .structure(TestStruct::new("Offset").field("Position", TestType::Class("Vector".into())))
        .structure(TestStruct::new("Vector").field("z", TestType::Single))
        .record(
            TestRecord::new(1, PORT, "Port.Sample", "ports/sample.xml").value(
                "AttachmentImplementation",
                TestValue::Struct(TestInstance::new(BONE).value("Helper", root)),
            ),
        )
        .structure(TestStruct::new("EntityClassDefinition"))
        .record(TestRecord::new(
            2,
            "EntityClassDefinition",
            "EntityClassDefinition.Item",
            "items/item.xml",
        ))
        .build()
}

fn definition(root: TestValue) -> Result<PortDefinition, Box<dyn Error>> {
    parse_definition(&port_fixture(root, TestType::String))
}

fn parse_definition(bytes: &[u8]) -> Result<PortDefinition, Box<dyn Error>> {
    let forge = DataForge::open(bytes)?;
    let tree = forge.records().next().ok_or("sample port")?.tree()?;
    Ok(port::read_definition(
        tree.root(),
        PortDefinitionPosition {
            record: tree.record_index(),
            component: 0,
            port: 0,
        },
    ))
}

#[test]
fn the_authored_subhelpers_keep_parentage_and_source_order() -> Result<(), Box<dyn Error>> {
    let definition = definition(nested_helpers("Heavy Variant"))?;
    assert_eq!(definition.root_helper_tags(), Some(""));
    assert_eq!(
        definition.transform().map(PortTransform::helper),
        Some("spine")
    );
    let children = definition.sub_helpers();
    assert_eq!(
        children
            .iter()
            .map(|row| (row.parent(), row.ordinal(), row.tags()))
            .collect::<Vec<_>>(),
        [
            (None, 1, Some("Heavy Variant")),
            (Some(0), 0, Some("Large")),
            (None, 2, Some("Heavy Variant")),
        ]
    );
    assert!(children[0].transform().is_none(), "only a tag is authored");
    let low = children[1].transform().ok_or("low helper")?;
    assert_eq!(low.attachment(), "Bone");
    assert_eq!(low.helper(), "spine_low");
    assert_eq!(low.offset_helper(), "item_origin");
    assert_eq!(low.position(), [0.0, 0.0, 0.25]);
    assert_eq!(low.offset_fields(), [true, false]);
    assert_eq!(
        children[2].transform().map(PortTransform::helper),
        Some("spine_high")
    );
    Ok(())
}

fn authored_literal(selector: &str, matching: &[&str]) -> PortHelperTagMatch {
    if matching.contains(&selector) {
        PortHelperTagMatch::Matched
    } else {
        PortHelperTagMatch::NotMatched
    }
}

#[test]
fn one_matching_nested_transform_keeps_original_source_positions() -> Result<(), Box<dyn Error>> {
    let definition = definition(nested_helpers("Other"))?;
    let selection = definition
        .select_helper(|selector| authored_literal(selector, &["Heavy Variant", "Large"]));
    let PortHelperChoice::Selected { transform, path } = selection.choice else {
        return Err("one matching nested helper".into());
    };
    assert_eq!(path.ordinals(), [1, 0], "original sparse child slots");
    assert_eq!(transform.helper(), "spine_low", "authored nested bone");
    assert_eq!(
        transform.offset_helper(),
        "item_origin",
        "authored item helper"
    );
    assert_eq!(transform.position(), [0.0, 0.0, 0.25], "authored offset");
    let observed: Vec<_> = selection
        .observations
        .iter()
        .map(|input| (input.path.ordinals(), input.text, input.evaluation))
        .collect();
    assert_eq!(
        observed,
        [
            (vec![], Some(""), None),
            (
                vec![1],
                Some("Heavy Variant"),
                Some(PortHelperTagMatch::Matched)
            ),
            (vec![1, 0], Some("Large"), Some(PortHelperTagMatch::Matched)),
            (vec![2], Some("Other"), Some(PortHelperTagMatch::NotMatched)),
        ],
        "the root and every inspected authored selector are retained"
    );
    Ok(())
}

#[test]
fn unmatched_ancestor_preserves_root_and_discards_its_descendant() -> Result<(), Box<dyn Error>> {
    let definition = definition(nested_helpers("Other"))?;
    let selection = definition.select_helper(|selector| authored_literal(selector, &["Large"]));
    let PortHelperChoice::Root(Some(transform)) = selection.choice else {
        return Err("root fallback after no matching branch".into());
    };
    assert_eq!(transform.helper(), "spine", "existing root fallback");
    let paths: Vec<_> = selection
        .observations
        .iter()
        .map(|input| input.path.ordinals())
        .collect();
    assert_eq!(
        paths,
        [vec![], vec![1], vec![2]],
        "fallback retains consulted selectors"
    );
    Ok(())
}

#[test]
fn equally_matched_branches_do_not_pick_first_or_deepest() -> Result<(), Box<dyn Error>> {
    let definition = definition(nested_helpers("Heavy Variant"))?;
    let selection = definition
        .select_helper(|selector| authored_literal(selector, &["Heavy Variant", "Large"]));
    let PortHelperChoice::Ambiguous(paths) = selection.choice else {
        return Err("both supported matched branches are competing".into());
    };
    let paths: Vec<_> = paths.iter().map(crate::PortHelperPath::ordinals).collect();
    assert_eq!(
        paths,
        [vec![1, 0], vec![2]],
        "both original candidates in source order"
    );
    Ok(())
}

#[test]
fn undecidable_selectors_and_unusable_matched_helpers_prevent_root_fallback()
-> Result<(), Box<dyn Error>> {
    let definition = definition(nested_helpers("Unsupported | Literal"))?;
    let unsupported = definition.select_helper(|text| match text {
        "Unsupported | Literal" => PortHelperTagMatch::Unsupported,
        _ => PortHelperTagMatch::NotMatched,
    });
    expect_unresolved(
        &unsupported.choice,
        PortHelperProblem::UnsupportedSelector,
        &[2],
    )?;
    let invalid = definition.select_helper(|text| authored_literal(text, &["Heavy Variant"]));
    expect_unresolved(&invalid.choice, PortHelperProblem::UnusableHelper, &[1])
}

#[test]
fn a_tagged_root_is_unresolved_until_a_root_selector_rule_is_known() -> Result<(), Box<dyn Error>> {
    let definition = definition(node("Fallback Tag", helper("spine"), Vec::new()))?;
    let selection = definition.select_helper(|_| PortHelperTagMatch::Matched);
    expect_unresolved(&selection.choice, PortHelperProblem::TaggedRoot, &[])?;
    let [root] = selection.observations.as_slice() else {
        return Err("the exact root selector evidence".into());
    };
    assert_eq!(root.text, Some("Fallback Tag"));
    assert_eq!(root.evaluation, None);
    Ok(())
}

#[test]
fn an_unreadable_selector_cannot_become_an_empty_selector() -> Result<(), Box<dyn Error>> {
    let root = TestValue::Struct(
        TestInstance::new(HELPER_NODE)
            .value("Tags", TestValue::UInt32(1))
            .value("Helper", helper("spine")),
    );
    let definition = parse_definition(&port_fixture(root, TestType::UInt32))?;
    let selection = definition.select_helper(|_| PortHelperTagMatch::Matched);
    expect_unresolved(
        &selection.choice,
        PortHelperProblem::UnreadableSelector,
        &[],
    )
}

fn expect_unresolved(
    choice: &PortHelperChoice<'_>,
    expected: PortHelperProblem,
    ordinals: &[usize],
) -> Result<(), Box<dyn Error>> {
    let PortHelperChoice::Unresolved { path, problem } = choice else {
        return Err("the helper cannot be chosen".into());
    };
    assert_eq!(
        *problem, expected,
        "explicit reason for unresolved selector"
    );
    assert_eq!(
        path.ordinals(),
        ordinals,
        "the exact unresolved source path"
    );
    Ok(())
}

#[test]
fn resolved_port_retains_root_and_reports_the_unproven_child() -> Result<(), Box<dyn Error>> {
    let root = node(
        "",
        helper("spine"),
        vec![
            TestValue::Null,
            node(
                "",
                TestValue::Null,
                vec![node("Heavy", helper("branch"), Vec::new())],
            ),
        ],
    );
    let bytes = port_fixture(root, TestType::String);
    let forge = DataForge::open(&bytes)?;
    let record = forge
        .records()
        .find(|record| {
            record
                .type_name()
                .is_ok_and(|name| name == "EntityClassDefinition")
        })
        .ok_or("the synthetic attached item")?;
    let item_record = record.index();
    let item = LoadoutResolver::new(&forge).resolve(&record)?;
    let definition = parse_definition(&bytes)?;
    let authored_position = definition.position();
    let mut port = PortNode {
        name: "backpack".to_owned(),
        name_proof: None,
        source: PortSource::Override,
        definition: Some(Arc::new(definition)),
        constraints: None,
        attachment: Attachment::Empty,
        nested: Vec::new(),
        visibility: PortVisibility::default(),
        origin: None,
    };
    assert!(
        port.placement_helper().diagnostic.is_none(),
        "an empty port has no item placement"
    );
    port.attachment = Attachment::Entity(Box::new(item));
    let resolution = port.placement_helper();
    assert_eq!(
        resolution.transform.map(PortTransform::helper),
        Some("spine"),
        "the legacy placement uses the root while the selector is unproven"
    );
    let diagnostic = resolution.diagnostic.ok_or("one unresolved helper")?;
    assert_eq!(diagnostic.port(), authored_position, "physical port");
    assert_eq!(
        diagnostic.attached_record_index(),
        Some(item_record),
        "actual resolved item"
    );
    assert_eq!(diagnostic.helper_ordinals(), [1, 0], "original nested path");
    assert_eq!(diagnostic.problem(), PortHelperProblem::UnsupportedSelector);
    Ok(())
}
