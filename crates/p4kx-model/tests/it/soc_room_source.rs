// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{SourceAttributePosition, SourceElementPosition};
use p4kx_model::{SocEntityFile, SocRoomRegistry, SocSegmentGeometry, SocSourceValue};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestChunkFile, TestElement};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ROOM: &str = "00000000-0000-0000-0000-000000000001";

#[test]
fn room_registry_preserves_original_slots_and_attribute_locations() -> TestResult {
    let xml = format!(
        r#"<EntityComponentRoom><Other/><Entity CryGUID="{ROOM}" BoundsCenter="-0,2,3" PivotRelativePos="malformed"/><Entity CryGUID="{ROOM}"/><Entity/></EntityComponentRoom>"#
    );
    let registry = SocRoomRegistry::parse(xml.as_bytes())?;
    let [first, repeated, missing] = registry
        .rooms()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "all original room slots")?;
    assert_eq!(
        [first.ordinal(), repeated.ordinal(), missing.ordinal()],
        [1, 2, 3],
        "the original first non-room element occupies a slot"
    );
    assert_eq!(
        first.bounds_center(),
        SocSourceValue::Present([-0.0, 2.0, 3.0]),
        "the registry centre remains in source coordinates"
    );
    assert_eq!(
        first.pivot_relative_position(),
        SocSourceValue::Invalid,
        "malformed authored pivot remains distinct"
    );
    assert_eq!(
        first.pivot_relative_rotation(),
        SocSourceValue::Absent,
        "missing pivot remains absent"
    );
    let guid = first
        .guid_attributes()
        .next()
        .ok_or("first physical GUID attribute")?;
    let SourceAttributePosition::Text(range) = guid.position() else {
        return Err("position in original text XML".into());
    };
    assert_eq!(
        xml.get(range.clone()),
        Some(format!("CryGUID=\"{ROOM}\"").as_str()),
        "the source span identifies the original attribute spelling"
    );
    assert_eq!(
        repeated
            .guid_attributes()
            .next()
            .map(|attribute| attribute.value()),
        Some(ROOM),
        "the exact original identifier is retained"
    );
    assert!(
        missing.guid_attributes().next().is_none(),
        "missing GUID declaration remains addressable"
    );
    Ok(())
}

#[test]
fn area_segments_keep_sparse_geometry_and_noncanonical_source_values() -> TestResult {
    let bytes = TestChunkFile::crch()
        .chunk(0x0001, 5, vec![0; 4])
        .chunk(0x0004, 3, test_entity_xml())
        .bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let chunk = file
        .entity_chunks()
        .next()
        .ok_or("positioned SOC entity chunk")??;
    assert_eq!(
        chunk.chunk_index(),
        1,
        "the earlier non-Entity chunk remains part of source position"
    );
    let entity = chunk.entities().next().ok_or("positioned SOC Entity")?;
    let [area, repeated] = entity
        .area_components()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "both area occurrences")?;
    assert_eq!(
        [area.ordinal(), repeated.ordinal()],
        [1, 2],
        "duplicate components keep physical positions"
    );
    let [link] = area
        .links()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "one room link")?;
    assert_eq!(
        link.ordinal(),
        (1, 1),
        "group and target ordinals use original sibling positions"
    );
    assert_eq!(
        link.guid_attributes()
            .next()
            .map(|attribute| attribute.value()),
        Some(ROOM),
        "the exact original identifier is retained"
    );
    check_segments(area)
}

fn check_segments(area: p4kx_model::SocAreaComponent<'_>) -> TestResult {
    let [outline, triangles, unknown, incomplete] = area
        .segments()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "four original segment occurrences")?;
    assert_eq!(
        [
            outline.ordinal(),
            triangles.ordinal(),
            unknown.ordinal(),
            incomplete.ordinal()
        ],
        [0, 2, 3, 4],
        "the non-segment child occupies its original slot"
    );
    assert_eq!(
        outline.geometry(),
        SocSegmentGeometry::Outline {
            height: SocSourceValue::Present(-3.5)
        },
        "negative height is preserved as authored"
    );
    let SocSourceValue::Present(transform) = outline.local_transform() else {
        return Err("stored Q/T transform".into());
    };
    assert_eq!(
        transform.rotation,
        [0.0, 0.0, 0.0, 2.0],
        "stored quaternion stays unnormalized"
    );
    assert_eq!(
        transform.translation,
        [3.0, 2.0, 1.0],
        "stored local translation is unchanged"
    );
    assert!(
        matches!(
            outline.element().position(),
            SourceElementPosition::CryXml(_)
        ),
        "the segment retains its CryXml node coordinate"
    );
    check_sparse_points(outline, triangles)?;
    check_invalid_segments(unknown, incomplete);
    Ok(())
}

fn check_invalid_segments(
    unknown: p4kx_model::SocRoomSegment<'_>,
    incomplete: p4kx_model::SocRoomSegment<'_>,
) {
    assert_eq!(
        unknown.geometry(),
        SocSegmentGeometry::Other {
            label: SocSourceValue::Present("FutureMesh")
        },
        "a future geometry discriminator remains available"
    );
    assert!(
        unknown
            .element()
            .elements()
            .any(|element| element.name() == "UnknownPayload"),
        "unknown geometry preserves its raw child"
    );
    assert_eq!(
        unknown.local_transform(),
        SocSourceValue::Absent,
        "no identity is synthesized for an absent transform"
    );
    assert_eq!(
        incomplete.geometry(),
        SocSegmentGeometry::Box {
            min: SocSourceValue::Present([9.0, 8.0, 7.0]),
            max: SocSourceValue::Invalid,
        },
        "one invalid bound does not discard the other bound"
    );
    assert_eq!(
        incomplete.local_transform(),
        SocSourceValue::Invalid,
        "a missing translation makes the declared transform invalid"
    );
}

fn check_sparse_points(
    outline: p4kx_model::SocRoomSegment<'_>,
    triangles: p4kx_model::SocRoomSegment<'_>,
) -> TestResult {
    let [first, malformed] = outline
        .outline_points()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "both outline point slots")?;
    assert_eq!(
        first.ordinal(),
        (Some(1), 0),
        "the point is in the first original group"
    );
    assert_eq!(
        malformed.ordinal(),
        (Some(2), 1),
        "the unrelated child still occupies a source slot"
    );
    assert_eq!(
        first.position(),
        SocSourceValue::Present([2.0, 3.0, 4.0]),
        "outline coordinates stay in the local frame"
    );
    assert_eq!(
        malformed.position(),
        SocSourceValue::Invalid,
        "malformed outline coordinates remain addressable"
    );
    assert_eq!(
        triangles.geometry(),
        SocSegmentGeometry::TriangleSoup,
        "sparse triangle storage retains its declared kind"
    );
    let points: Vec<_> = triangles
        .triangle_points()
        .map(|point| (point.ordinal(), point.position()))
        .collect();
    assert_eq!(
        points,
        [
            ((None, 0), SocSourceValue::Present([0.0, 0.0, 1.0])),
            ((None, 1), SocSourceValue::Absent)
        ],
        "a missing triangle coordinate retains its own ordered slot"
    );
    Ok(())
}

fn test_entity_xml() -> Vec<u8> {
    let area = TestElement::new("AreaComponent")
        .child(
            TestElement::new("Segment")
                .attribute("Type", "Shape")
                .attribute("Height", "-3.5")
                .attribute("transform_ls", "Q : 0,0,0,2 | T : 3,2,1")
                .child(TestElement::new("SoundData"))
                .child(
                    TestElement::new("Points")
                        .child(TestElement::new("Point").attribute("Pos", "2,3,4")),
                )
                .child(
                    TestElement::new("Points")
                        .child(TestElement::new("Other"))
                        .child(TestElement::new("Point").attribute("Pos", "wrong")),
                ),
        )
        .child(
            TestElement::new("Entities")
                .child(TestElement::new("Other"))
                .child(TestElement::new("Entity").attribute("EntityCryGUID", ROOM)),
        )
        .child(
            TestElement::new("Segment")
                .attribute("Type", "Trimesh")
                .child(TestElement::new("Point").attribute("pos", "0,0,1"))
                .child(TestElement::new("Point")),
        )
        .child(
            TestElement::new("Segment")
                .attribute("Type", "FutureMesh")
                .child(TestElement::new("UnknownPayload")),
        )
        .child(
            TestElement::new("Segment")
                .attribute("Type", "Box")
                .attribute("BoxMin", "9,8,7")
                .attribute("BoxMax", "bad")
                .attribute("transform_ls", "Q : 1,0,0,0"),
        );
    TestElement::new("Entities")
        .child(
            TestElement::new("Entity")
                .child(TestElement::new("Other"))
                .child(area)
                .child(TestElement::new("AreaComponent")),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little)
}
