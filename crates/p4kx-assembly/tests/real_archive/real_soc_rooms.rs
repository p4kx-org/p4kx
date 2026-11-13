// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::io::Read as _;

use p4kx_assembly::{SocRoomIndex, SocRoomTarget};
use p4kx_extraction::cryxml::{SourceDocument, SourceElement, SourceReadOptions};
use p4kx_extraction::{Archive, ArchiveWatch, Entry, Filter, Guid};
use p4kx_model::{
    SocAreaComponent, SocEntityFile, SocRoomRegistry, SocRoomSegment, SocSegmentGeometry,
    SocSegmentPoint, SocSegmentTransform, SocSourceValue,
};
use p4kx_testkit::{real_archive_key, real_archive_path};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

const REGISTRY: &str = "metadata/entitycomponentroom.xml";
const SHIP_CONTAINERS: &str = "Data/ObjectContainers/Ships/*.socpak";

#[derive(Default)]
struct Counts {
    streams: usize,
    chunks: usize,
    rooms: usize,
    linked_areas: usize,
    unmatched_areas: usize,
    boxes: usize,
    outlines: usize,
    triangles: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "only aggregate source coverage is reported"
)]
fn original_room_registry_and_three_segment_layouts_match_one_source_container() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let filter = Filter::new(SHIP_CONTAINERS);
    let mut candidates: Vec<_> = archive
        .entries()
        .iter()
        .filter(|entry| filter.matches(entry.path()))
        .collect();
    candidates.sort_by_key(|entry| entry.uncompressed_size());
    for candidate in candidates {
        watch
            .poll()
            .map_err(|_| "Archive changed during container selection")?;
        let Ok(container) = archive.open_container(candidate) else {
            continue;
        };
        let mut registries = container
            .entries()
            .iter()
            .filter(|entry| registry_path(entry.path()));
        let Some(registry) = registries.next() else {
            continue;
        };
        if registries.next().is_some() {
            continue;
        }
        let counts = compare(&container, registry)?;
        if counts.boxes == 0 || counts.outlines == 0 || counts.triangles == 0 {
            continue;
        }
        watch
            .check()
            .map_err(|_| "Archive changed during room verification")?;
        println!(
            "SOC rooms: {} streams, {} chunks, {} declarations, {} linked areas, {} unmatched areas, {} boxes, {} outlines, {} triangle soups",
            counts.streams,
            counts.chunks,
            counts.rooms,
            counts.linked_areas,
            counts.unmatched_areas,
            counts.boxes,
            counts.outlines,
            counts.triangles
        );
        return Ok(());
    }
    Err("no readable original room registry with all three segment shapes".into())
}

fn registry_path(path: &str) -> bool {
    let path = path.replace('\\', "/");
    path.eq_ignore_ascii_case(REGISTRY)
        || path.rsplit_once('/').is_some_and(|(folder, leaf)| {
            leaf.eq_ignore_ascii_case("entitycomponentroom.xml")
                && folder
                    .rsplit('/')
                    .next()
                    .is_some_and(|name| name.eq_ignore_ascii_case("metadata"))
        })
}

fn compare(container: &Archive, entry: &Entry) -> TestResult<Counts> {
    let raw = bytes(container, entry)?;
    let source = SourceDocument::parse(&raw, SourceReadOptions::default())?;
    let registry = SocRoomRegistry::parse(&raw)?;
    assert!(
        !source.is_lossy() && !registry.document().is_lossy(),
        "registry XML fully readable"
    );
    let guid = |text: &str| text.parse::<Guid>().map_err(|_| "original registry GUID");
    let original: Vec<_> = source
        .root()
        .elements()
        .enumerate()
        .filter(|(_, node)| node.name() == "Entity")
        .collect();
    let mut ids = BTreeSet::new();
    let mut counts = Counts::default();
    for (observed, (ordinal, raw)) in registry.rooms().zip(original.iter().copied()) {
        assert_eq!(
            observed.ordinal(),
            ordinal,
            "room declaration uses the original sibling slot"
        );
        assert_eq!(
            observed.element().position(),
            raw.position(),
            "room declaration retains the original XML coordinate"
        );
        let authored = raw.attribute("CryGUID").ok_or("original room GUID")?;
        assert!(
            observed
                .guid_attributes()
                .next()
                .is_some_and(|attribute| attribute.value() == authored),
            "exact raw room GUID attribute"
        );
        ids.insert(guid(authored)?);
        counts.rooms += 1;
    }
    assert_eq!(
        counts.rooms,
        original.len(),
        "every original registry occurrence"
    );
    let index = SocRoomIndex::new(&registry);
    for stream in container
        .entries()
        .iter()
        .filter(|entry| entry.path().to_ascii_lowercase().ends_with(".soc"))
    {
        compare_stream(&bytes(container, stream)?, &index, &ids, &mut counts)?;
        counts.streams += 1;
    }
    Ok(counts)
}

fn compare_stream(
    bytes: &[u8],
    index: &SocRoomIndex<'_>,
    ids: &BTreeSet<Guid>,
    counts: &mut Counts,
) -> TestResult {
    let source = SocEntityFile::parse(bytes)?;
    for chunk in source.entity_chunks() {
        let chunk = chunk?;
        assert!(
            !chunk.document().is_lossy(),
            "SOC Entity source fully readable"
        );
        counts.chunks += 1;
        for entity in chunk.entities() {
            let raw_areas: Vec<_> = entity
                .element()
                .elements()
                .enumerate()
                .filter(|(_, node)| node.name() == "AreaComponent")
                .collect();
            assert_eq!(
                entity.area_components().count(),
                raw_areas.len(),
                "every original AreaComponent occurrence"
            );
            for (area, (ordinal, raw)) in entity.area_components().zip(raw_areas) {
                assert_eq!(
                    area.ordinal(),
                    ordinal,
                    "area uses its original sibling slot"
                );
                assert_eq!(
                    area.element().position(),
                    raw.position(),
                    "area retains the original XML coordinate"
                );
                let linked = area.links().any(|link| {
                    matches!(
                        index.target(link),
                        SocRoomTarget::Matched { .. } | SocRoomTarget::Ambiguous { .. }
                    )
                });
                let original_link = raw
                    .elements()
                    .filter(|node| node.name() == "Entities")
                    .flat_map(SourceElement::elements)
                    .filter_map(|node| node.attribute("EntityCryGUID"))
                    .filter_map(|guid| guid.parse().ok())
                    .any(|guid| ids.contains(&guid));
                assert_eq!(linked, original_link, "exact registry GUID join");
                if linked {
                    counts.linked_areas += 1;
                    compare_segments(area, counts)?;
                } else {
                    counts.unmatched_areas += 1;
                }
            }
        }
    }
    Ok(())
}

fn compare_segments(area: SocAreaComponent<'_>, counts: &mut Counts) -> TestResult {
    let raw: Vec<_> = area
        .element()
        .elements()
        .enumerate()
        .filter(|(_, node)| node.name() == "Segment")
        .collect();
    assert_eq!(
        area.segments().count(),
        raw.len(),
        "every original Segment occurrence"
    );
    for (segment, (ordinal, raw)) in area.segments().zip(raw) {
        assert_eq!(
            segment.ordinal(),
            ordinal,
            "segment uses its original sibling slot"
        );
        assert_eq!(
            segment.element().position(),
            raw.position(),
            "segment retains the original XML coordinate"
        );
        compare_transform(raw.attribute("transform_ls"), segment.local_transform())?;
        compare_shape(segment, raw, counts)?;
    }
    Ok(())
}

fn compare_shape(
    segment: SocRoomSegment<'_>,
    raw: SourceElement<'_>,
    counts: &mut Counts,
) -> TestResult {
    match (raw.attribute("Type"), segment.geometry()) {
        (Some("Box"), SocSegmentGeometry::Box { min, max }) => {
            assert_eq!(
                min,
                SocSourceValue::Present(vector(raw.attribute("BoxMin"))?),
                "exact original BoxMin"
            );
            assert_eq!(
                max,
                SocSourceValue::Present(vector(raw.attribute("BoxMax"))?),
                "exact original BoxMax"
            );
            counts.boxes += 1;
        }
        (Some("Shape"), SocSegmentGeometry::Outline { height }) => {
            let expected = raw
                .attribute("Height")
                .ok_or("original outline height")?
                .parse()?;
            assert_eq!(
                height,
                SocSourceValue::Present(expected),
                "exact original outline height"
            );
            let raw_points = raw
                .elements()
                .filter(|node| node.name() == "Points")
                .flat_map(SourceElement::elements)
                .filter(|node| node.name() == "Point");
            compare_points(segment.outline_points(), raw_points, "Pos")?;
            counts.outlines += 1;
        }
        (Some("Trimesh"), SocSegmentGeometry::TriangleSoup) => {
            let raw_points = raw.elements().filter(|node| node.name() == "Point");
            compare_points(segment.triangle_points(), raw_points, "pos")?;
            counts.triangles += 1;
        }
        (
            Some(label),
            SocSegmentGeometry::Other {
                label: SocSourceValue::Present(original),
            },
        ) if label == original => {}
        _ => return Err("original segment shape was not interpreted".into()),
    }
    Ok(())
}

fn compare_transform(
    source: Option<&str>,
    actual: SocSourceValue<SocSegmentTransform>,
) -> TestResult {
    let Some(source) = source else {
        assert_eq!(
            actual,
            SocSourceValue::Absent,
            "missing local transform stays absent"
        );
        return Ok(());
    };
    let parts: Vec<_> = source.split('|').map(str::trim).collect();
    let [rotation, translation] = parts.as_slice() else {
        return Err("original segment transform has Q/T parts".into());
    };
    let SocSourceValue::Present(actual) = actual else {
        return Err("original segment Q/T resolves".into());
    };
    let rotation = rotation
        .strip_prefix('Q')
        .ok_or("original Q")?
        .trim_start()
        .strip_prefix(':');
    let translation = translation
        .strip_prefix('T')
        .ok_or("original T")?
        .trim_start()
        .strip_prefix(':');
    assert_eq!(
        actual.rotation,
        vector(rotation)?,
        "exact stored quaternion"
    );
    assert_eq!(
        actual.translation,
        vector(translation)?,
        "exact stored translation"
    );
    Ok(())
}

fn compare_points<'document>(
    actual: impl Iterator<Item = SocSegmentPoint<'document>>,
    original: impl Iterator<Item = SourceElement<'document>>,
    attribute: &str,
) -> TestResult {
    let original: Vec<_> = original.collect();
    let actual: Vec<_> = actual.collect();
    assert_eq!(actual.len(), original.len(), "every original point slot");
    for (point, raw) in actual.into_iter().zip(original) {
        assert_eq!(
            point.element().position(),
            raw.position(),
            "point retains the original XML coordinate"
        );
        let expected = vector(raw.attribute(attribute))?;
        assert_eq!(
            point.position(),
            SocSourceValue::Present(expected),
            "point retains exact authored coordinate values"
        );
    }
    Ok(())
}

fn vector<const N: usize>(source: Option<&str>) -> TestResult<[f64; N]> {
    let parts = source
        .ok_or("original authored vector")?
        .split(',')
        .map(|value| value.trim().parse())
        .collect::<Result<Vec<_>, _>>()?;
    parts
        .try_into()
        .map_err(|_| "original vector dimension differs".into())
}

fn bytes(archive: &Archive, entry: &Entry) -> TestResult<Vec<u8>> {
    let mut bytes = Vec::new();
    archive
        .open_entry_verified(entry)
        .map_err(|_| "open original SOC source Entry")?
        .reject_overlong()
        .read_to_end(&mut bytes)
        .map_err(|_| "read complete original SOC source Entry")?;
    Ok(bytes)
}
