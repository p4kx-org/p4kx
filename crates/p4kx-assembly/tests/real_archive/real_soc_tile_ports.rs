// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocContainerCapture, SocMemberSelection, SocTilePortIndex, SocTilePortTarget};
use p4kx_extraction::{Archive, ArchiveWatch, CancelToken};
use p4kx_model::{SocContainerDescriptor, SocEntityFile, SocSourceValue};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::real_soc_rooms::{TestResult, bytes, ship_containers};

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "only aggregate source coverage is reported"
)]
fn original_tile_attachment_resolves_to_a_physical_descriptor_offset() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open installed archive")?;
    let watch = ArchiveWatch::new(&archive);
    for entry in ship_containers(&archive) {
        watch.poll().map_err(|_| "installed Archive changed")?;
        let Ok(container) = archive.open_container(entry) else {
            continue;
        };
        if !has_local_tile_ports(&container, entry.path())? {
            continue;
        }
        let capture = SocContainerCapture::read(&[&archive, &container], &CancelToken::new())?;
        for occurrence in capture.occurrences() {
            let matched = compare_ports(occurrence)?;
            if matched > 0 {
                watch.check().map_err(|_| "installed Archive changed")?;
                println!("SOC tile ports: {matched} original attachment offsets verified");
                return Ok(());
            }
        }
    }
    Err("no selected source contains a matched tile interaction offset".into())
}

fn compare_ports(occurrence: &p4kx_assembly::SocContainerOccurrence) -> TestResult<usize> {
    let Some(descriptor) = occurrence.parsed_descriptor()? else {
        return Ok(0);
    };
    if descriptor.tile_item_ports().next().is_none() {
        return Ok(0);
    }
    let index = SocTilePortIndex::read(&descriptor);
    let mut matched = 0;
    for stream in &occurrence.streams {
        let Some(member) = stream
            .member
            .as_ref()
            .and_then(SocMemberSelection::selected)
        else {
            continue;
        };
        let payload = member
            .payload()
            .map_err(|_| "verified original SOC stream")?;
        let file = SocEntityFile::parse(payload)?;
        for chunk in file.entity_chunks() {
            for entity in chunk?.entities() {
                for attachment in entity.tile_attachments() {
                    let SocTilePortTarget::Matched(site) = index.resolve(attachment) else {
                        continue;
                    };
                    let SocSourceValue::Present(offset) = site.offset else {
                        continue;
                    };
                    let original = descriptor
                        .tile_item_ports()
                        .find(|port| port.element().position() == site.position)
                        .ok_or("physical item port declaration")?;
                    let expected = original
                        .element()
                        .attribute("interactionOffset")
                        .ok_or("authored port offset attribute")?;
                    let parts = expected
                        .split(',')
                        .map(|text| text.trim().parse::<f64>())
                        .collect::<Result<Vec<_>, _>>()?;
                    assert!(
                        offset
                            .iter()
                            .zip(&parts)
                            .all(|(found, expected)| found.to_bits() == expected.to_bits())
                            && parts.len() == offset.len(),
                        "exact original interaction offset"
                    );
                    matched += 1;
                }
            }
        }
    }
    Ok(matched)
}

fn has_local_tile_ports(container: &Archive, archive_path: &str) -> TestResult<bool> {
    let file = archive_path
        .rsplit(['/', '\\'])
        .next()
        .ok_or("container file")?;
    let (stem, _) = file.rsplit_once('.').ok_or("container stem")?;
    let direct = format!("{stem}.xml");
    let nested = format!("{stem}\\{stem}.xml");
    for entry in container.entries() {
        let path = entry.path().replace('/', "\\");
        if !path.eq_ignore_ascii_case(&direct) && !path.eq_ignore_ascii_case(&nested) {
            continue;
        }
        let raw = bytes(container, entry)?;
        let descriptor = SocContainerDescriptor::parse(&raw)?;
        if descriptor.tile_item_ports().next().is_some() {
            return Ok(true);
        }
    }
    Ok(false)
}
