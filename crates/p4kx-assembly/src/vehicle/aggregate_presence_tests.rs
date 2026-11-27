// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceLocation, SourceUseRole};

use super::support::{self, TestResult};
use crate::{
    VehicleImplementation, VehicleTotalFieldPresence, VehicleTotalFieldUse, VehicleTotalMetric,
};

const BASE: &[u8] = br#"<Vehicle><Parts>
    <Part id="NoPort" mass="1" damageMax="1"/>
    <Part mass="2" damageMax="2"><iTeMpOrT id="Missing"/><ItemPort flags="invisible"/></Part>
    <Part mass="3" damageMax="4"><ItemPort flags=""/></Part>
    <Part mass="4" damageMax="8"><ItemPort id="Cleared" flags="invisible"/></Part>
</Parts><Modifications><Modification name="Observed"><Elems>
    <Elem idRef="Cleared" name="flags"/>
</Elems></Modification></Modifications></Vehicle>"#;

#[test]
fn effective_item_port_flags_presence_keeps_exact_absence_and_assignment_sites() -> TestResult {
    let (_directory, archive) = support::archive(BASE, None)?;
    support::with_archive(&archive, "Observed", true, |vehicle, observer| {
        let flags = selected_flags(vehicle);
        let expected = [
            (
                "textXmlElement",
                r#"<Part id="NoPort" mass="1" damageMax="1"/>"#,
                None,
            ),
            (
                "textXmlElement",
                r#"<iTeMpOrT id="Missing"/>"#,
                Some("flags"),
            ),
            ("textXmlAttribute", r#"flags="""#, Some("flags")),
            (
                "textXmlElement",
                r#"<Elem idRef="Cleared" name="flags"/>"#,
                Some("value"),
            ),
        ];
        let parsed = match observer
            .parsed
            .lock()
            .map_err(|_| "poisoned parse witness")?
            .as_slice()
        {
            [only] => only.clone(),
            _ => return Err("exactly one original parse".into()),
        };
        for (usage, (expected_namespace, expected_text, expected_field)) in
            flags.into_iter().zip(expected)
        {
            let site = usage
                .site()
                .ok_or("original flags or child-scope coordinates")?;
            assert_eq!(usage.role(), SourceUseRole::Selector);
            assert_eq!(usage.evidence().is_some(), expected_field.is_some());
            assert!(site.file().is_same_operation(&parsed));
            assert_xml_location(
                site.location(),
                expected_namespace,
                expected_text,
                expected_field,
            )?;
        }
        Ok(())
    })?;

    support::with_archive(&archive, "Observed", false, |vehicle, _| {
        for usage in selected_flags(vehicle) {
            assert_eq!(usage.role(), SourceUseRole::Attempted);
            assert!(usage.site().is_none());
            assert!(usage.evidence().is_none());
        }
        Ok(())
    })
}

fn assert_xml_location(
    location: Option<&SourceLocation>,
    expected_namespace: &str,
    expected_text: &str,
    expected_field: Option<&str>,
) -> TestResult {
    let Some(SourceLocation::Subresource {
        namespace,
        indices,
        field,
    }) = location
    else {
        return Err("original XML location".into());
    };
    assert_eq!(namespace, expected_namespace);
    if let Some(expected_field) = expected_field {
        assert_eq!(field, expected_field);
    }
    let [start, end] = indices.as_slice() else {
        return Err("original byte range".into());
    };
    assert_eq!(
        std::str::from_utf8(&BASE[usize::try_from(*start)?..usize::try_from(*end)?])?,
        expected_text,
    );
    Ok(())
}

fn selected_flags(vehicle: &VehicleImplementation) -> Vec<VehicleTotalFieldUse<'_>> {
    use VehicleTotalFieldPresence::{Absent, Present};

    let mut hull_uses = Vec::new();
    let totals = vehicle.totals_with_field_uses(|usage| {
        if usage.metric() == VehicleTotalMetric::HullHealth {
            hull_uses.push(usage);
        }
    });
    assert_eq!(totals, vehicle.totals());
    assert_eq!(totals.mass(), 10.0);
    assert_eq!(totals.health(), 15.0);

    let (by_part, remainder) = hull_uses.as_chunks::<3>();
    assert!(remainder.is_empty());
    let flags: Vec<_> = by_part.iter().map(|fields| fields[2]).collect();
    assert_eq!(
        flags
            .iter()
            .map(|usage| usage.presence())
            .collect::<Vec<_>>(),
        [Absent, Absent, Present, Present],
    );
    flags
}
