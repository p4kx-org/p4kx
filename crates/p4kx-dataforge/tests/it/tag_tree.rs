// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The tag tree of a synthetic DataForge: paths, parents, children, issues, and the annotations of
//! tag References in Record XML and Record JSON.

use std::error::Error;

use p4kx_dataforge::{
    Annotations, DataForge, Guid, JsonLayout, RecordJson, RecordXml, TagResolver, TagTree,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

type TestResult = Result<(), Box<dyn Error>>;

const DATABASE: u128 = 0x00d0_0000_0000_4000_8000_0000_0000_0001;
const A: u128 = 0x00a0_0000_0000_4000_8000_0000_0000_0001;
const A1: u128 = 0x00a1_0000_0000_4000_8000_0000_0000_0001;
const A2: u128 = 0x00a2_0000_0000_4000_8000_0000_0000_0001;
const A1A: u128 = 0x00a3_0000_0000_4000_8000_0000_0000_0001;
const B: u128 = 0x00b0_0000_0000_4000_8000_0000_0000_0001;
const ORPHAN: u128 = 0x00c0_0000_0000_4000_8000_0000_0000_0001;
const ORPHAN_CHILD: u128 = 0x00c1_0000_0000_4000_8000_0000_0000_0001;
const LOOP_X: u128 = 0x00e0_0000_0000_4000_8000_0000_0000_0001;
const LOOP_Y: u128 = 0x00e1_0000_0000_4000_8000_0000_0000_0001;
const MISSING: u128 = 0x00ff_0000_0000_4000_8000_0000_0000_0001;
const HOLDER: u128 = 0x0100_0000_0000_4000_8000_0000_0000_0001;
const PATH: &str = "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml";

fn guid(value: u128) -> Result<Guid, Box<dyn Error>> {
    let text = format!("{value:032x}");
    let dashed = format!(
        "{}-{}-{}-{}-{}",
        &text[0..8],
        &text[8..12],
        &text[12..16],
        &text[16..20],
        &text[20..32]
    );
    Ok(dashed.parse()?)
}

fn references(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().map(|g| TestValue::Reference(*g)).collect())
}

fn tag(guid: u128, name: &str, legacy: u32, children: &[u128]) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), PATH)
        .value("tagName", TestValue::text(name))
        .value("legacyGUID", TestValue::UInt32(legacy))
        .value("children", references(children))
}

fn builder() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", TestType::ReferenceTo("Tag".into())),
        )
        .structure(
            TestStruct::new("TagDatabase").array("tags", TestType::ReferenceTo("Tag".into())),
        )
        .structure(
            TestStruct::new("Holder")
                .array("requiredTags", TestType::ReferenceTo("Tag".into()))
                .field("owner", TestType::Reference)
                .field("label", TestType::Locale),
        )
        .record(
            TestRecord::new(DATABASE, "TagDatabase", "TagDatabase.TagDatabase", PATH)
                .value("tags", references(&[A, B, A, MISSING])),
        )
        .record(tag(A, "Weapon", 7, &[A1, A2, MISSING]))
        .record(tag(A1, "Gun", u32::MAX, &[A1A]))
        .record(tag(A2, "Missile", 9, &[A1A]))
        .record(tag(A1A, "Ballistic", 3, &[]))
        .record(tag(B, "Armour", 4, &[]))
        .record(tag(ORPHAN, "Stray", 1, &[ORPHAN_CHILD]))
        .record(tag(ORPHAN_CHILD, "Leaf", 2, &[]))
        .record(tag(LOOP_X, "LoopX", 5, &[LOOP_Y]))
        .record(tag(LOOP_Y, "LoopY", 6, &[LOOP_X]))
        .record(
            TestRecord::new(
                HOLDER,
                "Holder",
                "Holder.Sample",
                "libs/foundry/records/holder.xml",
            )
            .value("requiredTags", references(&[A1A, ORPHAN_CHILD, MISSING]))
            .value("owner", TestValue::Reference(HOLDER))
            .value("label", TestValue::text("@label")),
        )
}

fn with_tree<T>(
    check: impl FnOnce(&DataForge<'_>, &TagTree) -> Result<T, Box<dyn Error>>,
) -> Result<T, Box<dyn Error>> {
    let bytes = builder().build();
    let forge = DataForge::open(&bytes)?;
    let tree = TagTree::build(&forge);
    check(&forge, &tree)
}

fn paths(tree: &TagTree) -> Vec<&str> {
    tree.iter().map(|tag| tag.path()).collect()
}

#[test]
fn the_tree_lists_tags_in_pre_order_with_full_paths() -> TestResult {
    with_tree(|_, tree| {
        assert_eq!(
            paths(tree),
            [
                "Weapon",
                "Weapon.Gun",
                "Weapon.Gun.Ballistic",
                "Weapon.Missile",
                "Armour",
                "Stray",
                "Stray.Leaf",
                "LoopX",
                "LoopX.LoopY",
            ],
            "database roots first, then unlisted ones; a tag stays where it was listed first"
        );
        assert_eq!(tree.len(), 9, "all nine Tag Records");
        Ok(())
    })
}

#[test]
fn a_tag_knows_its_parent_children_depth_and_legacy_id() -> TestResult {
    with_tree(|_, tree| {
        let gun = tree.get(guid(A1)?).ok_or("no Gun")?;
        assert_eq!(gun.name(), "Gun", "the name alone");
        assert_eq!(gun.path(), "Weapon.Gun", "the full path");
        assert_eq!(gun.depth(), 1, "one below the root");
        assert_eq!(gun.legacy_id(), None, "4294967295 means none");
        assert_eq!(gun.parent().map(|tag| tag.path()), Some("Weapon"));
        let children: Vec<_> = gun.children().map(|tag| tag.name()).collect();
        assert_eq!(children, ["Ballistic"], "children in listed order");
        let weapon = tree.get(guid(A)?).ok_or("no Weapon")?;
        assert_eq!(weapon.legacy_id(), Some(7), "the stored number");
        assert!(weapon.parent().is_none(), "a root has no parent");
        let below: Vec<_> = weapon.descendants().map(|tag| tag.path()).collect();
        assert_eq!(
            below,
            ["Weapon.Gun", "Weapon.Gun.Ballistic", "Weapon.Missile"],
            "descendants in pre-order"
        );
        let ancestors: Vec<_> = tree
            .get(guid(A1A)?)
            .ok_or("no Ballistic")?
            .ancestors_and_self()
            .map(|tag| tag.name())
            .collect();
        assert_eq!(ancestors, ["Ballistic", "Gun", "Weapon"], "up to the root");
        Ok(())
    })
}

#[test]
fn is_within_tells_a_tag_below_another() -> TestResult {
    with_tree(|_, tree| {
        let weapon = tree.get(guid(A)?).ok_or("no Weapon")?;
        let ballistic = tree.get(guid(A1A)?).ok_or("no Ballistic")?;
        let armour = tree.get(guid(B)?).ok_or("no Armour")?;
        assert!(ballistic.is_within(&weapon), "below at any depth");
        assert!(weapon.is_within(&weapon), "a tag is within itself");
        assert!(!weapon.is_within(&ballistic), "not the other way round");
        assert!(!armour.is_within(&weapon), "a sibling root is not below");
        Ok(())
    })
}

#[test]
fn roots_are_the_database_tags_then_the_unlisted_ones() -> TestResult {
    with_tree(|_, tree| {
        let roots: Vec<_> = tree
            .roots()
            .map(|tag| (tag.name(), tag.is_listed()))
            .collect();
        assert_eq!(
            roots,
            [
                ("Weapon", true),
                ("Armour", true),
                ("Stray", false),
                ("LoopX", false),
            ],
            "unlisted tags no tag lists come before a cycle"
        );
        Ok(())
    })
}

#[test]
fn issues_count_what_the_database_does_not_hold_together() -> TestResult {
    with_tree(|_, tree| {
        let issues = tree.issues();
        assert_eq!(
            issues.dangling(),
            2,
            "a root and a child that are no Tag Record"
        );
        assert_eq!(
            issues.repeated(),
            3,
            "Weapon listed twice, Ballistic under two tags, LoopX again below LoopY"
        );
        assert_eq!(issues.unlisted(), 4, "Stray, Leaf, LoopX and LoopY");
        assert_eq!(issues.unreadable(), 0, "every Tag Record reads");
        assert!(!issues.is_clean());
        Ok(())
    })
}

#[test]
fn a_cycle_is_cut_where_it_closes() -> TestResult {
    with_tree(|_, tree| {
        let x = tree.get(guid(LOOP_X)?).ok_or("no LoopX")?;
        let y = tree.get(guid(LOOP_Y)?).ok_or("no LoopY")?;
        assert_eq!(y.parent(), Some(x), "LoopY below LoopX");
        assert_eq!(y.children().len(), 0, "LoopX is not LoopY's child again");
        Ok(())
    })
}

#[test]
fn positions_count_the_pre_order_and_a_subtree_follows_its_tag() -> TestResult {
    with_tree(|_, tree| {
        let gun = tree.get(guid(A1)?).ok_or("no Gun")?;
        assert_eq!(gun.position(), 1, "after Weapon");
        assert_eq!(tree.at(1), Some(gun), "at() is the inverse of position()");
        let weapon = tree.at(0).ok_or("no first tag")?;
        assert_eq!(weapon.descendants().len(), 3, "Gun, Ballistic and Missile");
        assert_eq!(tree.at(4).map(|tag| tag.path()), Some("Armour"));
        assert!(tree.at(tree.len()).is_none(), "past the last tag");
        for tag in tree.iter() {
            assert_eq!(tree.at(tag.position()), Some(tag), "every position");
        }
        Ok(())
    })
}

#[test]
fn an_unknown_guid_has_no_tag() -> TestResult {
    with_tree(|_, tree| {
        assert!(tree.get(guid(MISSING)?).is_none(), "no such Tag Record");
        assert!(
            tree.path_of(guid(HOLDER)?).is_none(),
            "a Record of another type"
        );
        assert_eq!(
            tree.tag_path(guid(A)?),
            Some("Weapon"),
            "the resolver trait"
        );
        Ok(())
    })
}

#[test]
fn a_dataforge_without_tags_gives_an_empty_tree() -> TestResult {
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Item").field("name", TestType::String))
        .build();
    let forge = DataForge::open(&bytes)?;
    let tree = TagTree::build(&forge);

    assert!(tree.is_empty(), "no tags");
    assert!(tree.issues().is_clean(), "nothing to complain about");
    assert_eq!(tree.roots().count(), 0, "no roots");
    Ok(())
}

fn holder_xml(annotations: Annotations<'_>) -> Result<String, Box<dyn Error>> {
    with_tree(|forge, _| {
        let tree = forge
            .record_by_guid(guid(HOLDER)?)
            .ok_or("no Holder")?
            .tree()?;
        let mut out = Vec::new();
        RecordXml::new().write_annotated(&tree, annotations, &mut out)?;
        Ok(String::from_utf8(out)?)
    })
}

fn holder_json(annotations: Annotations<'_>) -> Result<String, Box<dyn Error>> {
    with_tree(|forge, _| {
        let tree = forge
            .record_by_guid(guid(HOLDER)?)
            .ok_or("no Holder")?
            .tree()?;
        let mut out = Vec::new();
        RecordJson::new(JsonLayout::Compact).write_annotated(&tree, annotations, &mut out)?;
        Ok(String::from_utf8(out)?)
    })
}

#[test]
fn without_tags_the_output_is_that_of_the_plain_writers() -> TestResult {
    let plain = with_tree(|forge, _| {
        let tree = forge
            .record_by_guid(guid(HOLDER)?)
            .ok_or("no Holder")?
            .tree()?;
        let (mut xml, mut json) = (Vec::new(), Vec::new());
        tree.write_xml(&mut xml)?;
        RecordJson::new(JsonLayout::Compact).write(&tree, &mut json)?;
        Ok((String::from_utf8(xml)?, String::from_utf8(json)?))
    })?;

    assert_eq!(holder_xml(Annotations::none())?, plain.0, "XML");
    assert_eq!(holder_json(Annotations::none())?, plain.1, "JSON");
    assert!(!plain.0.contains("tag="), "no tag attribute by default");
    assert!(!plain.1.contains("\"tag\""), "no tag key by default");
    Ok(())
}

#[test]
fn xml_annotates_tag_references_only() -> TestResult {
    let bytes = builder().build();
    let forge = DataForge::open(&bytes)?;
    let tags = TagTree::build(&forge);
    let xml = holder_xml(Annotations::none().with_tags(Some(&tags)))?;

    let wanted = |guid: u128, path: &str| {
        format!(
            "<p4kx:reference guid=\"{}\" path=\"libs/foundry/records/tagdatabase/tagdatabase.tagdatabase\" type=\"Tag\" tag=\"{path}\"/>",
            guid_text(guid)
        )
    };
    assert!(xml.contains(&wanted(A1A, "Weapon.Gun.Ballistic")), "{xml}");
    assert!(xml.contains(&wanted(ORPHAN_CHILD, "Stray.Leaf")), "{xml}");
    assert!(
        xml.contains("type=\"Holder\"/>"),
        "a Reference to another type carries no tag: {xml}"
    );
    assert_eq!(xml.matches(" tag=").count(), 2, "{xml}");
    roxmltree::Document::parse(&xml)?;
    Ok(())
}

#[test]
fn json_annotates_tag_references_only() -> TestResult {
    let bytes = builder().build();
    let forge = DataForge::open(&bytes)?;
    let tags = TagTree::build(&forge);
    let json = holder_json(Annotations::none().with_tags(Some(&tags)))?;

    let value: serde_json::Value = serde_json::from_str(&json)?;
    let required = value["requiredTags"].as_array().ok_or("no array")?;
    assert_eq!(required.len(), 3, "the missing Record stays an error value");
    assert_eq!(required[0]["tag"], "Weapon.Gun.Ballistic");
    assert_eq!(required[1]["tag"], "Stray.Leaf");
    assert!(value["owner"].get("tag").is_none(), "{json}");
    Ok(())
}

fn guid_text(value: u128) -> String {
    guid(value).map(|guid| guid.to_string()).unwrap_or_default()
}
