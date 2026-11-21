// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceAttributePosition;
use p4kx_model::{SocContainerDescriptor, SocPhysicsDeclaration, SocPhysicsKind, SocSourceValue};
use p4kx_testkit::{CryXmlByteOrder, CryXmlSignature, TestElement};

type Check = Result<(), Box<dyn std::error::Error>>;

#[test]
fn grid_sections_keep_ownership_slot_order_and_empty_groups() -> Check {
    let xml = br#"<ObjectContainer gridRadius="-0">
      <Other/>
      <PhysicsGridPart shape="voxel"><PhysicsGridVoxel filename="objects/test/a.cgf"/></PhysicsGridPart>
      <CargoGrid><PhysicsGridPart/></CargoGrid>
      <PhysicsGrid gravity="0, 1.5, -9.8" inheritGravity="0" gridType="local" cellSize="2" hasPart="1" portalExclusiveMode="0" extra="retained">
        <PhysicsGridChildParts/>
        <PhysicsGridChildParts>
          <Other/>
          <PhysicsGridPart shape="voxel"><PhysicsGridVoxel filename=""/></PhysicsGridPart>
          <PhysicsGridPart/>
        </PhysicsGridChildParts>
      </PhysicsGrid>
    </ObjectContainer>"#;
    let source = SocContainerDescriptor::parse(xml)?;
    let physics = source.location_physics();
    let radius = physics.grid_radius();
    let SocSourceValue::Present(value) = radius.into_value() else {
        return Err("authored grid radius".into());
    };
    assert!(
        value.is_sign_negative(),
        "signed zero survives typed parsing"
    );
    let raw = radius.attributes().next().ok_or("grid radius attribute")?;
    let SourceAttributePosition::Text(span) = raw.position() else {
        return Err("original text position".into());
    };
    assert_eq!(
        xml.get(span.clone()),
        Some(b"gridRadius=\"-0\"".as_slice()),
        "original text span"
    );

    let declarations: Vec<_> = physics.declarations().collect();
    let [part, grid] = declarations.as_slice() else {
        return Err("two direct source declarations".into());
    };
    assert_eq!(
        [part.ordinal(), grid.ordinal()],
        [1, 3],
        "original root sibling indices"
    );
    assert_eq!(part.kind(), SocPhysicsKind::Part, "standalone root part");
    assert_eq!(
        part.owner().position(),
        source.document().root().position(),
        "physical part owner"
    );
    check_controls(*grid)?;
    check_child_groups(*grid)
}

fn check_controls(grid: SocPhysicsDeclaration<'_>) -> Check {
    let controls = grid.grid().ok_or("typed grid settings")?;
    assert_eq!(
        controls.gravity().into_value(),
        SocSourceValue::Present([0.0, 1.5, -9.8]),
        "the gravity vector retains its original component order"
    );
    assert_eq!(
        controls.cell_size().into_value(),
        SocSourceValue::Present(2.0),
        "the cell size is a scalar"
    );
    assert_eq!(
        controls.grid_type().into_value(),
        SocSourceValue::Present("local"),
        "grid type label"
    );
    assert_eq!(
        [
            controls.inherit_gravity().into_value(),
            controls.has_part().into_value()
        ],
        [
            SocSourceValue::Present(false),
            SocSourceValue::Present(true)
        ],
        "explicit switch states"
    );
    assert_eq!(
        controls.portal_exclusive_mode().into_value(),
        SocSourceValue::Present(0),
        "zero portal mode"
    );
    assert!(
        grid.element()
            .attributes()
            .any(|attr| attr.name() == "extra"),
        "unknown attribute remains accessible"
    );
    Ok(())
}

fn check_child_groups(grid: SocPhysicsDeclaration<'_>) -> Check {
    let groups: Vec<_> = grid.children().collect();
    let [empty, nested] = groups.as_slice() else {
        return Err("two child-part group occurrences".into());
    };
    assert_eq!(
        [empty.ordinal(), nested.ordinal()],
        [0, 1],
        "child-group order"
    );
    assert_eq!(
        empty.kind(),
        SocPhysicsKind::ChildParts,
        "original empty group declaration"
    );
    assert_eq!(
        empty.children().count(),
        0,
        "empty group has no synthetic children"
    );
    let parts: Vec<_> = nested.children().collect();
    let [nested_part, second_part] = parts.as_slice() else {
        return Err("two nested declarations".into());
    };
    assert_eq!(
        [nested_part.ordinal(), second_part.ordinal()],
        [1, 2],
        "unrelated nodes occupy sibling slots"
    );
    assert_eq!(
        nested_part.owner().position(),
        nested.element().position(),
        "group owns nested part"
    );
    let voxel = nested_part.children().next().ok_or("nested voxel")?;
    assert_eq!(
        voxel.owner().position(),
        nested_part.element().position(),
        "part owns nested voxel"
    );
    let filename = voxel.filename().ok_or("voxel path")?;
    assert_eq!(
        filename.into_value(),
        SocSourceValue::Present(""),
        "empty string remains an original value"
    );
    Ok(())
}

#[test]
fn missing_and_malformed_fields_are_independent_of_cargo() -> Check {
    let source = SocContainerDescriptor::parse(
        br#"<ObjectContainer><PhysicsGrid gravity="0, NaN, -9.8" cellSize="unknown" hasPart="7" gridType=""/><CargoGrid><PhysicsGrid/></CargoGrid></ObjectContainer>"#,
    )?;
    let physics = source.location_physics();
    assert_eq!(
        physics.grid_radius().into_value(),
        SocSourceValue::Absent,
        "missing root control"
    );
    let mut rows = physics.declarations();
    let grid = rows
        .next()
        .and_then(SocPhysicsDeclaration::grid)
        .ok_or("grid")?;
    assert_eq!(
        grid.gravity().into_value(),
        SocSourceValue::Invalid,
        "a nonfinite component invalidates the vector"
    );
    assert_eq!(
        grid.cell_size().into_value(),
        SocSourceValue::Invalid,
        "a malformed scalar remains invalid"
    );
    assert_eq!(
        grid.has_part().into_value(),
        SocSourceValue::Invalid,
        "malformed boolean"
    );
    assert_eq!(
        grid.grid_type().into_value(),
        SocSourceValue::Invalid,
        "empty symbolic control"
    );
    assert_eq!(
        grid.inherit_gravity().into_value(),
        SocSourceValue::Absent,
        "missing boolean"
    );
    assert!(
        rows.next().is_none(),
        "CargoGrid is a distinct declaration family"
    );
    Ok(())
}

#[test]
fn repeated_cryxml_controls_keep_both_original_attribute_sites() -> Check {
    let bytes = TestElement::new("ObjectContainer")
        .child(
            TestElement::new("PhysicsGrid")
                .attribute("hasPart", "0")
                .attribute("hasPart", "1"),
        )
        .to_cryxml(CryXmlSignature::CryXmlB, CryXmlByteOrder::Little);
    let descriptor = SocContainerDescriptor::parse(&bytes)?;
    let grid = descriptor
        .location_physics()
        .declarations()
        .next()
        .and_then(SocPhysicsDeclaration::grid)
        .ok_or("original grid control")?;
    let has_part = grid.has_part();
    assert_eq!(
        has_part.into_value(),
        SocSourceValue::Invalid,
        "repetition is not a unique control"
    );
    let positions: Vec<_> = has_part
        .attributes()
        .map(|attr| attr.position().clone())
        .collect();
    assert_eq!(
        positions,
        [
            SourceAttributePosition::CryXml {
                node_index: 1,
                attribute_index: 0
            },
            SourceAttributePosition::CryXml {
                node_index: 1,
                attribute_index: 1
            },
        ],
        "both original CryXml attribute positions"
    );
    Ok(())
}
