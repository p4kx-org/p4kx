// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocEntityIndex, SocEntityTarget};
use p4kx_model::SocEntityFile;
use p4kx_testkit::TestChunkFile;

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ENTITY_CHUNK: u32 = 0x0004;
const FOUND: &str = "11111111-1111-1111-1111-111111111111";
const DUPLICATE: &str = "22222222-2222-2222-2222-222222222222";
const MISSING: &str = "33333333-3333-3333-3333-333333333333";

fn streams() -> (Vec<u8>, Vec<u8>) {
    let source = format!(
        r#"<Entities><Metadata/><Entity EntityCryGUID="{DUPLICATE}"><EntityLinks><Link/><Link TargetCryGUID="bad"/><Link TargetCryGUID="{FOUND}"/><Link TargetCryGUID="{DUPLICATE}"/><Link TargetCryGUID="{MISSING}"/></EntityLinks></Entity></Entities>"#
    );
    let target = format!(
        r#"<Entities><Entity EntityCryGUID="{FOUND}"/><Entity EntityCryGUID="{DUPLICATE}"/></Entities>"#
    );
    let source_bytes = TestChunkFile::crch()
        .chunk(ENTITY_CHUNK, 3, source.into_bytes())
        .bytes();
    let target_bytes = TestChunkFile::crch()
        .chunk(ENTITY_CHUNK, 3, target.into_bytes())
        .bytes();
    (source_bytes, target_bytes)
}

#[test]
fn authored_door_links_use_only_physical_container_owners() -> TestResult {
    let (source_bytes, target_bytes) = streams();
    let source_file = SocEntityFile::parse(&source_bytes)?;
    let target_file = SocEntityFile::parse(&target_bytes)?;
    let source = source_file.entity_chunks().next().ok_or("source chunk")??;
    let target = target_file.entity_chunks().next().ok_or("target chunk")??;
    let group = source
        .entities()
        .next()
        .and_then(|entity| entity.entity_links().next())
        .ok_or("original links")?;
    let [absent, invalid, found, duplicate, missing] = group
        .links()
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| "five original links")?;
    let mut container = SocEntityIndex::new();
    container.add_chunk(5, &source);
    assert!(
        matches!(container.target(found), SocEntityTarget::Unmatched { .. }),
        "outside this physical scope before insertion"
    );
    container.add_chunk(11, &target);
    assert!(
        matches!(container.target(absent), SocEntityTarget::Absent),
        "absent source attribute"
    );
    assert!(
        matches!(container.target(invalid), SocEntityTarget::Invalid),
        "malformed source attribute"
    );
    assert!(
        matches!(container.target(missing), SocEntityTarget::Unmatched { .. }),
        "valid GUID with no indexed owner"
    );
    let SocEntityTarget::Matched { site, .. } = container.target(found) else {
        return Err("expected one cross-stream physical owner".into());
    };
    assert_eq!(
        (site.stream_index, site.chunk_index, site.entity_ordinal),
        (11, 0, 0),
        "cross-stream physical position"
    );
    let SocEntityTarget::Ambiguous { sites, .. } = container.target(duplicate) else {
        return Err("expected ambiguous physical owners".into());
    };
    let slots: Vec<_> = sites
        .iter()
        .map(|site| (site.stream_index, site.entity_ordinal))
        .collect();
    assert_eq!(
        slots,
        [(5, 1), (11, 1)],
        "both physical owners retain original positions"
    );
    Ok(())
}
