// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_model::{SocEntityComponentKind, SocEntityFile, SocInlineLoadoutKind, SocSourceValue};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestElement};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ENTITIES: u32 = 0x0004;
const MANUAL: &str = "SItemPortLoadoutManualParams";

fn bytes(root: &TestElement) -> Vec<u8> {
    TestChunkFile::crch()
        .chunk(
            ENTITIES,
            3,
            root.to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little),
        )
        .bytes()
}

fn door_fixture() -> TestElement {
    TestElement::new("Entities").child(
        TestElement::new("Entity")
            .attribute("EntityCryGUID", "synthetic owner")
            .attribute("EntityClassGUID", "synthetic class")
            .child(
                TestElement::new("PropertiesDataCore")
                    .child(TestElement::new("SCItemDoor"))
                    .child(
                        TestElement::new("specialWeakPointers")
                            .child(TestElement::new("SCItemProximitySensor")),
                    )
                    .child(TestElement::new("EntityComponentRoomConnector")),
            )
            .child(TestElement::new("InteractionLinkController"))
            .child(
                TestElement::new("EntityLinks")
                    .child(TestElement::new("Unknown"))
                    .child(TestElement::new("Link").attribute("Name", ""))
                    .child(
                        TestElement::new("Link")
                            .attribute("Name", "duplicate")
                            .attribute("Name", "duplicate")
                            .attribute("TargetCryGUID", "first")
                            .attribute("TargetCryGUID", "second"),
                    ),
            )
            .child(TestElement::new("EntityLinks")),
    )
}

#[test]
fn authored_door_source_preserves_owner_scope_and_link_occurrences() -> TestResult {
    let bytes = bytes(&door_fixture());
    let file = SocEntityFile::parse(&bytes)?;
    let chunk = file.entity_chunks().next().ok_or("source chunk")??;
    let entity = chunk.entities().next().ok_or("source entity")?;
    let roles: Vec<_> = entity
        .authored_components()
        .map(|component| (component.ordinal(), component.kind()))
        .collect();
    assert_eq!(
        roles,
        [
            ((Some(0), 0), SocEntityComponentKind::Door),
            ((Some(0), 2), SocEntityComponentKind::RoomConnector),
            ((None, 1), SocEntityComponentKind::InteractionLinkController),
        ],
        "only supported direct declarations in physical document order"
    );
    let [group, empty] = entity
        .entity_links()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "two groups")?;
    assert_eq!(
        (group.ordinal(), empty.ordinal()),
        (2, 3),
        "source group ordinals"
    );
    assert!(empty.links().next().is_none(), "explicit empty link group");
    let [first, second] = group
        .links()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "two links")?;
    assert_eq!(
        (first.ordinal(), second.ordinal()),
        (1, 2),
        "unclassified slot preserved"
    );
    assert_eq!(
        first.name(),
        SocSourceValue::Present(""),
        "explicit empty name"
    );
    assert_eq!(
        second.name(),
        SocSourceValue::Invalid,
        "repeated name is ambiguous"
    );
    assert_eq!(
        second
            .target_attributes()
            .map(|attribute| attribute.value())
            .collect::<Vec<_>>(),
        ["first", "second"],
        "original duplicate target attributes are retained"
    );
    Ok(())
}

fn manual_loadout_fixture() -> TestElement {
    let nested = TestElement::new("loadout")
        .attribute("__type", MANUAL)
        .child(TestElement::new("entries"));
    let entry = TestElement::new("SItemPortLoadoutEntryParams")
        .attribute("itemPortName", "test port")
        .attribute("entityClassName", "test class")
        .attribute("entityClassReference", "test class reference")
        .child(TestElement::new("Decoration"))
        .child(nested);
    TestElement::new("Entities").child(
        TestElement::new("Entity").child(
            TestElement::new("PropertiesDataCore").child(
                TestElement::new("EntityComponentDefaultLoadout")
                    .child(
                        TestElement::new("loadout")
                            .attribute("__type", MANUAL)
                            .child(
                                TestElement::new("entries")
                                    .child(TestElement::new("Decoration"))
                                    .child(entry),
                            ),
                    )
                    .child(TestElement::new("loadout").attribute("__type", MANUAL))
                    .child(TestElement::new("loadout").attribute("__type", "<NullPtr>"))
                    .child(TestElement::new("loadout").attribute("__type", "Unsupported"))
                    .child(TestElement::new("loadout"))
                    .child(TestElement::new("loadout").attribute("__type", ""))
                    .child(
                        TestElement::new("loadout")
                            .attribute("__type", MANUAL)
                            .attribute("__type", MANUAL),
                    ),
            ),
        ),
    )
}

#[test]
fn authored_door_source_keeps_loadout_omission_empty_and_unknown_distinct() -> TestResult {
    use SocInlineLoadoutKind::{Manual, Null, Other};
    use SocSourceValue::{Absent, Invalid, Present};

    let bytes = bytes(&manual_loadout_fixture());
    let file = SocEntityFile::parse(&bytes)?;
    let chunk = file.entity_chunks().next().ok_or("source chunk")??;
    let entity = chunk.entities().next().ok_or("source entity")?;
    let component = entity.authored_components().next().ok_or("loadout owner")?;
    let declarations: Vec<_> = component.loadouts().collect();
    let kinds: Vec<_> = declarations.iter().map(|source| source.kind()).collect();
    assert_eq!(
        kinds,
        [
            Present(Manual),
            Present(Manual),
            Present(Null),
            Present(Other("Unsupported")),
            Absent,
            Invalid,
            Invalid
        ],
        "each authored discriminant remains distinct"
    );
    let (Some(authored), Some(omitted)) = (declarations.first(), declarations.get(1)) else {
        return Err("expected manual declarations".into());
    };
    assert!(
        omitted.entry_groups().next().is_none(),
        "entries was omitted"
    );
    let group = authored.entry_groups().next().ok_or("authored entries")?;
    let entry = group.entries().next().ok_or("authored entry")?;
    assert_eq!(
        entry.ordinal(),
        1,
        "unclassified sibling still takes its slot"
    );
    assert_eq!(entry.port_name(), Present("test port"), "authored port");
    assert_eq!(
        entry.class_name(),
        Present("test class"),
        "authored class name"
    );
    let reference = entry
        .class_reference_attributes()
        .next()
        .ok_or("authored class reference")?;
    assert_eq!(
        reference.value(),
        "test class reference",
        "original class reference"
    );
    verify_explicit_nested_entries(entry)
}

fn verify_explicit_nested_entries(entry: p4kx_model::SocInlineLoadoutEntry<'_>) -> TestResult {
    let nested = entry.loadouts().next().ok_or("authored nested override")?;
    assert_eq!(nested.ordinal(), 1, "original nested slot");
    let nested_group = nested
        .entry_groups()
        .next()
        .ok_or("explicit nested entries")?;
    assert!(
        nested_group.entries().next().is_none(),
        "explicit empty nested entries"
    );
    Ok(())
}
