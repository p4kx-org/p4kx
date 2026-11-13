// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceElement;
use p4kx_extraction::{Archive, ArchiveWatch};
use p4kx_model::{LightState, SocBakedLightSource, SocEntityFile, SocLightSource, SocSourceValue};
use p4kx_testkit::{real_archive_key, real_archive_path};

use super::real_soc_rooms::{TestResult, bytes, ship_containers};

#[derive(Default)]
struct Coverage {
    lights: usize,
    groups: usize,
    modes: usize,
    authored_zero_default: usize,
    zero_default_with_positive_other: usize,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
#[expect(
    clippy::print_stdout,
    reason = "only aggregate source coverage is reported"
)]
fn original_light_modes_and_groups_keep_physical_order_and_authored_zero() -> TestResult {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open installed archive")?;
    let watch = ArchiveWatch::new(&archive);
    for entry in ship_containers(&archive) {
        watch.poll().map_err(|_| "installed Archive changed")?;
        let Ok(container) = archive.open_container(entry) else {
            continue;
        };
        let coverage = compare_container(&container)?;
        if coverage.lights == 0 || coverage.groups == 0 || coverage.authored_zero_default == 0 {
            continue;
        }
        watch.check().map_err(|_| "installed Archive changed")?;
        println!(
            "SOC lights: {} components, {} groups, {} modes, {} explicit zero defaults, {} with an active alternate",
            coverage.lights,
            coverage.groups,
            coverage.modes,
            coverage.authored_zero_default,
            coverage.zero_default_with_positive_other
        );
        return Ok(());
    }
    Err("no selected source container has both light groups and authored zero intensity".into())
}

fn compare_container(container: &Archive) -> TestResult<Coverage> {
    let mut coverage = Coverage::default();
    for member in container.entries() {
        if !member
            .path()
            .rsplit_once('.')
            .is_some_and(|(_, suffix)| suffix.eq_ignore_ascii_case("soc"))
        {
            continue;
        }
        let data = bytes(container, member)?;
        let file = SocEntityFile::parse(&data)?;
        for chunk in file.entity_chunks() {
            for entity in chunk?.entities() {
                for light in entity.authored_lights() {
                    compare_modes(light, &mut coverage)?;
                }
                for group in entity.authored_light_groups() {
                    coverage.groups += 1;
                    for light in group.lights().flat_map(SocBakedLightSource::components) {
                        compare_modes(light, &mut coverage)?;
                    }
                }
            }
        }
    }
    Ok(coverage)
}

fn compare_modes(light: SocLightSource<'_>, coverage: &mut Coverage) -> TestResult {
    coverage.lights += 1;
    let original = light.element().elements().filter_map(|element| {
        let kind = match element.name() {
            "defaultState" => LightState::Default,
            "auxiliaryState" => LightState::Auxiliary,
            "emergencyState" => LightState::Emergency,
            "cinematicState" => LightState::Cinematic,
            _ => return None,
        };
        Some((kind, element))
    });
    let mut zero_default = false;
    let mut active_other = false;
    let mut projected = light.modes();
    for (kind, element) in original {
        let actual = projected.next().ok_or("every original mode is present")?;
        let intensity = original_intensity(element);
        assert_eq!(actual.kind(), kind, "authored mode kind");
        assert_eq!(
            actual.element().position(),
            element.position(),
            "original mode position"
        );
        assert_eq!(
            actual.intensity(),
            intensity,
            "exact authored intensity state"
        );
        coverage.modes += 1;
        zero_default |= kind == LightState::Default && intensity == SocSourceValue::Present(0.0);
        active_other |= kind != LightState::Default
            && matches!(intensity, SocSourceValue::Present(value) if value > 0.0);
    }
    assert!(
        projected.next().is_none(),
        "only authored modes are projected"
    );
    coverage.authored_zero_default += usize::from(zero_default);
    coverage.zero_default_with_positive_other += usize::from(zero_default && active_other);
    Ok(())
}

fn original_intensity(element: SourceElement<'_>) -> SocSourceValue<f32> {
    let mut attributes = element
        .attributes()
        .filter(|attr| attr.name() == "intensity");
    match (attributes.next(), attributes.next()) {
        (None, _) => SocSourceValue::Absent,
        (Some(_), Some(_)) => SocSourceValue::Invalid,
        (Some(attribute), None) => attribute
            .value()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
            .map_or(SocSourceValue::Invalid, SocSourceValue::Present),
    }
}
