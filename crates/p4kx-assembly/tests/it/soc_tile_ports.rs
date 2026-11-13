// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocTilePortIndex, SocTilePortTarget};
use p4kx_model::{SocContainerDescriptor, SocEntityFile, SocSourceValue};
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
