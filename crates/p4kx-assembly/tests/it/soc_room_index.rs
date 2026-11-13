// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocRoomGuid, SocRoomIndex, SocRoomTarget};
use p4kx_model::{SocEntityFile, SocRoomRegistry};
use p4kx_testkit::TestChunkFile;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const SHARED: &str = "01234567-89ab-cdef-0123-456789abcdef";
const UNMATCHED: &str = "01234567-89ab-cdef-0123-456789abcdee";

#[test]
fn room_targets_keep_original_candidates_within_each_physical_registry() -> TestResult {
    let repeated_xml = format!(
        r#"<EntityComponentRoom><Entity/><Entity CryGUID="{SHARED}"/><Entity CryGUID="bad"/><Entity CryGUID="{SHARED}"/></EntityComponentRoom>"#
    );
    let nested_xml =
        format!(r#"<EntityComponentRoom><Entity CryGUID="{SHARED}"/></EntityComponentRoom>"#);
    let repeated = SocRoomRegistry::parse(repeated_xml.as_bytes())?;
    let nested = SocRoomRegistry::parse(nested_xml.as_bytes())?;
    let repeated_index = SocRoomIndex::new(&repeated);
    let nested_index = SocRoomIndex::new(&nested);
    assert!(
        matches!(
            repeated
                .rooms()
                .next()
                .map(|room| SocRoomGuid::read(room.guid_attributes())),
            Some(SocRoomGuid::Absent)
        ),
        "a registry entry without a GUID retains the absent state"
    );
    let bytes = link_fixture();
    let soc = SocEntityFile::parse(&bytes)?;
    let chunk = soc.entity_chunks().next().ok_or("original SOC chunk")??;
    let area = chunk
        .entities()
        .next()
        .and_then(|entity| entity.area_components().next())
        .ok_or("original area")?;
    let [shared, unmatched, absent, malformed] = area
        .links()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "four original targets")?;
    let SocRoomTarget::Ambiguous { rooms, .. } = repeated_index.target(shared) else {
        return Err("the two physical room declarations must remain ambiguous".into());
    };
    assert_eq!(
        rooms.iter().map(|room| room.ordinal()).collect::<Vec<_>>(),
        [1, 3],
        "both duplicate declarations retain their source order"
    );
    assert!(
        matches!(nested_index.target(shared), SocRoomTarget::Matched { room, .. } if room.ordinal() == 0),
        "the same GUID resolves only within the selected registry"
    );
    assert!(
        matches!(
            nested_index.target(unmatched),
            SocRoomTarget::Unmatched { .. }
        ),
        "well-formed target without a declaration remains unmatched"
    );
    assert!(
        matches!(nested_index.target(absent), SocRoomTarget::Absent),
        "missing link target stays absent"
    );
    assert!(
        matches!(nested_index.target(malformed), SocRoomTarget::Invalid),
        "malformed link target stays invalid"
    );
    Ok(())
}

fn link_fixture() -> Vec<u8> {
    let xml = format!(
        r#"<Entities><Entity><AreaComponent><Entities><Entity EntityCryGUID="{}"/><Entity EntityCryGUID="{UNMATCHED}"/><Entity/><Entity EntityCryGUID="bad"/></Entities></AreaComponent></Entity></Entities>"#,
        SHARED.to_ascii_uppercase()
    );
    TestChunkFile::crch()
        .chunk(0x0004, 3, xml.into_bytes())
        .bytes()
}
