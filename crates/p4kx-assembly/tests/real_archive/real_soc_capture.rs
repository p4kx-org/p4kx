// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocContainerCapture, SocMemberSelection};
use p4kx_extraction::{Archive, CancelToken};
use p4kx_model::{SocEntityFile, SocRoomRegistry};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::real_soc_rooms::{TestResult, registry_path, ship_containers};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "only aggregate source coverage is reported"
)]
fn descriptor_scoped_capture_preserves_admitted_soc_sources() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open configured Archive")?;
    for candidate in ship_containers(&archive) {
        let Ok(container) = archive.open_container(candidate) else {
            continue;
        };
        if !container
            .entries()
            .iter()
            .any(|entry| registry_path(entry.path()))
        {
            continue;
        }
        let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())
            .map_err(|_| "capture original descriptor sources")?;
        let Some(root) = capture.occurrences().first() else {
            return Err("selected container must own a root occurrence".into());
        };
        let Some(registry) = root.registry.selected() else {
            continue;
        };
        let rooms = SocRoomRegistry::parse(registry.payload().map_err(|_| "verified registry")?)?
            .rooms()
            .count();
        let Some(descriptor) = root.parsed_descriptor()? else {
            continue;
        };
        assert_eq!(
            root.streams.len(),
            descriptor.streams().count(),
            "all original stream declarations"
        );
        assert_eq!(
            root.children.len(),
            descriptor.children().count(),
            "all original child declarations"
        );
        let (streams, entities) = stream_counts(root, &container)?;
        if rooms == 0 || streams == 0 || entities == 0 {
            continue;
        }
        println!(
            "SOC capture: {} logical occurrences, {rooms} room declarations, {streams} original streams, {entities} entities",
            capture.occurrences().len()
        );
        return Ok(());
    }
    Err("no current container with a readable descriptor, registry and entity stream".into())
}

fn stream_counts(
    root: &p4kx_assembly::SocContainerOccurrence,
    container: &Archive,
) -> TestResult<(usize, usize)> {
    let mut entities = 0;
    let mut streams = 0;
    for stream in &root.streams {
        let Some(member) = stream
            .member
            .as_ref()
            .and_then(SocMemberSelection::selected)
        else {
            continue;
        };
        assert!(
            container.owns_origin(member.origin()),
            "original container owns every selected stream"
        );
        let file = SocEntityFile::parse(member.payload().map_err(|_| "verified stream")?)?;
        for chunk in file.entity_chunks() {
            entities += chunk?.entities().count();
        }
        streams += 1;
    }
    Ok((streams, entities))
}
