// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    SocTileLightAnchor, SocTileLightAnchorError, SocTilePortIndex, SocTilePortTarget,
};
use p4kx_model::{SocContainerDescriptor, SocEntityFile, SocSourceValue, Transform};
use p4kx_testkit::TestChunkFile;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const DESCRIPTOR: &[u8] = br#"<ObjectContainer>
<TileXmlEntry><TileItemPortEntries>
<ItemPort name="a" interactionOffset="1,2,3"/>
<ItemPort name="b"/>
<ItemPort name="dup" interactionOffset="0,0,0"/>
<Other/>
<ItemPort name="dup" interactionOffset="4,5,6"/>
<ItemPort name="bad" interactionOffset="1,no,3"/>
</TileItemPortEntries></TileXmlEntry>
<ItemPort name="outside" interactionOffset="7,8,9"/>
</ObjectContainer>"#;

const ENTITIES: &[u8] = br#"<Entities><Entity>
<TileAttachmentParams ItemPortName="a"/><TileAttachmentParams ItemPortName="b"/>
<TileAttachmentParams ItemPortName="dup"/><TileAttachmentParams ItemPortName="bad"/>
<TileAttachmentParams ItemPortName="outside"/><TileAttachmentParams/>
<TileAttachmentParams ItemPortName=""/>
</Entity></Entities>"#;

#[test]
fn tile_selectors_keep_original_port_sites_and_offset_states() -> TestResult {
    let descriptor = SocContainerDescriptor::parse(DESCRIPTOR)?;
    let index = SocTilePortIndex::read(&descriptor);
    let bytes = TestChunkFile::crch().chunk(0x0004, 3, ENTITIES).bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let chunk = file.entity_chunks().next().ok_or("Entity chunk")??;
    let entity = chunk.entities().next().ok_or("Entity")?;
    let targets = entity
        .tile_attachments()
        .map(|attachment| index.resolve(attachment))
        .collect::<Vec<_>>();
    let [
        SocTilePortTarget::Matched(valid),
        SocTilePortTarget::Matched(absent),
        SocTilePortTarget::Ambiguous(conflicting),
        SocTilePortTarget::Matched(invalid),
        SocTilePortTarget::Unmatched,
        SocTilePortTarget::Absent,
        SocTilePortTarget::Invalid,
    ] = targets.as_slice()
    else {
        return Err("original port target states differ".into());
    };
    assert_eq!(
        valid.offset,
        SocSourceValue::Present([1.0, 2.0, 3.0]),
        "exact owner-frame translation"
    );
    assert_eq!(
        absent.offset,
        SocSourceValue::Absent,
        "missing offset is absent"
    );
    assert_eq!(
        invalid.offset,
        SocSourceValue::Invalid,
        "malformed offset is invalid"
    );
    let original = descriptor
        .tile_item_ports()
        .filter(|port| port.name() == SocSourceValue::Present("dup"))
        .map(|port| (port.element().position(), port.interaction_offset()));
    assert!(
        conflicting
            .iter()
            .map(|site| (site.position.clone(), site.offset))
            .eq(original),
        "duplicate original ports retain both coordinates and values"
    );
    Ok(())
}

#[test]
fn tile_anchor_replaces_only_the_origin_in_each_container_frame() -> TestResult {
    let descriptor = SocContainerDescriptor::parse(DESCRIPTOR)?;
    let index = SocTilePortIndex::read(&descriptor);
    let target = first_target(&index)?;
    let owner = Transform {
        rows: [
            [0.0, -2.0, 0.0, 5.0],
            [4.0, 0.0, 0.0, -10.0],
            [0.0, 0.0, 0.5, 2.0],
        ],
    };
    let group = Transform {
        rows: [
            [0.0, -1.0, 0.0, 9.0],
            [1.0, 0.0, 0.0, 8.0],
            [0.0, 0.0, 1.0, 7.0],
        ],
    };
    let local_light = Transform {
        rows: [
            [1.0, 0.0, 0.0, 1.0],
            [0.0, 1.0, 0.0, 2.0],
            [0.0, 0.0, 1.0, 3.0],
        ],
    };
    let resolved = SocTileLightAnchor::resolve(target, Some(owner), Some(group))?;
    let expected = Transform {
        rows: [
            [0.0, -1.0, 0.0, 3.0],
            [1.0, 0.0, 0.0, 2.0],
            [0.0, 0.0, 1.0, 2.0],
        ],
    };
    assert_eq!(resolved.authored(), group, "original frame");
    assert_eq!(resolved.anchored(), expected, "only group origin changes");
    assert_eq!(
        owner
            .then(&resolved.anchored())
            .translation()
            .map(f32::to_bits),
        [1.0_f32, 2.0, 3.0].map(f32::to_bits),
        "corrected origin maps to the original owner-frame port"
    );
    assert_eq!(
        resolved
            .anchored()
            .then(&local_light)
            .translation()
            .map(f32::to_bits),
        [1.0_f32, 3.0, 5.0].map(f32::to_bits),
        "the light still follows its original group-relative transform"
    );
    let reused = SocTileLightAnchor::resolve(target, Some(Transform::IDENTITY), Some(group))?;
    assert_eq!(
        reused.anchored().translation().map(f32::to_bits),
        [1.0_f32, 2.0, 3.0].map(f32::to_bits),
        "a second container occurrence gets its own transform"
    );
    Ok(())
}

#[test]
fn tile_anchor_requires_a_group_and_an_invertible_container_frame() -> TestResult {
    let descriptor = SocContainerDescriptor::parse(DESCRIPTOR)?;
    let index = SocTilePortIndex::read(&descriptor);
    let target = first_target(&index)?;
    let singular = Transform {
        rows: [[0.0; 4]; 3],
    };
    assert_eq!(
        SocTileLightAnchor::resolve(target, Some(singular), Some(Transform::IDENTITY)),
        Err(SocTileLightAnchorError::InvalidContainerFrame),
        "singular parent is not anchored"
    );
    assert_eq!(
        SocTileLightAnchor::resolve(target, Some(Transform::IDENTITY), None),
        Err(SocTileLightAnchorError::MissingGroupFrame),
        "the anchor cannot invent a group orientation"
    );
    Ok(())
}

fn first_target<'index>(
    index: &'index SocTilePortIndex<'_>,
) -> Result<SocTilePortTarget<'index>, Box<dyn std::error::Error>> {
    let bytes = TestChunkFile::crch().chunk(0x0004, 3, ENTITIES).bytes();
    let file = SocEntityFile::parse(&bytes)?;
    let chunk = file.entity_chunks().next().ok_or("Entity chunk")??;
    let entity = chunk.entities().next().ok_or("Entity")?;
    let attachment = entity.tile_attachments().next().ok_or("tile attachment")?;
    Ok(index.resolve(attachment))
}
