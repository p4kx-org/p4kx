// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutChanges;
use p4kx_testkit::{TestEntry, fitted_paint_vehicle_entries};
use serde_json::Value;

use super::{BASE_MATERIAL, PORT, assemble_entries, field_selectors, paint_evidence, string_at};
use crate::assembly_support::TestResult;

const XML_FOLDER: &str = "Data/Scripts/Entities/Vehicles/Implementations/Xml";
const EXPLICIT: &str = "Fixture_Paint_Tags.xml";
const FALLBACK: &str = "TEST_Paint_Red.xml";
const BASE: &[u8] = br#"<Vehicle name="TEST_Paint"><Parts/>
  <Modifications><Modification name="Red" patchFile="Modifications/Fixture_Paint_Tags.xml"/></Modifications>
</Vehicle>"#;
const PATCH: &[u8] = br#"<Modifications><Parts>
  <Part name="hardpoint_livery" class="ItemPort">
    <ItemPort id="livery" requiredTags="Family_Red" portTags="FixturePortOffer">
      <Types><Type type="Paints"/></Types>
    </ItemPort>
  </Part>
</Parts></Modifications>"#;
const UNRELATED_BASE: &[u8] = br#"<Vehicle name="TEST_Paint"><Parts>
  <Part name="hardpoint_livery" class="ItemPort">
    <ItemPort id="livery" requiredTags="Family_Red" portTags="FixturePortOffer">
      <Types><Type type="Paints"/></Types>
    </ItemPort>
  </Part>
</Parts><Modifications><Modification name="Red" patchFile="Modifications/Fixture_Paint_Tags.xml"/></Modifications></Vehicle>"#;
const UNRELATED_PATCH: &[u8] = b"<Modifications><Helpers/></Modifications>";

#[test]
fn fitted_paint_retains_the_exact_route_to_its_used_vehicle_patch() -> TestResult {
    for (base, file, patch, tag_encryption, routed) in [
        (BASE, EXPLICIT, PATCH, "unencrypted", true),
        (BASE, FALLBACK, PATCH, "unencrypted", false),
        (
            UNRELATED_BASE,
            EXPLICIT,
            UNRELATED_PATCH,
            "encrypted",
            false,
        ),
    ] {
        let mut entries = fitted_paint_vehicle_entries();
        let base_path = format!("{XML_FOLDER}/TEST_Paint.xml");
        entries.retain(|entry| entry.path() != base_path);
        entries.push(TestEntry::stored(base_path, base).encrypted());
        entries.push(TestEntry::stored(
            format!("{XML_FOLDER}/Modifications/{file}"),
            patch,
        ));
        let changes = LoadoutChanges::new().replace(PORT, "TEST_Paint_Red");
        let assembled = assemble_entries("TEST_Hull_Red", &changes, entries, Some(BASE_MATERIAL))?;
        let report = paint_evidence(&assembled)?;
        let sources = field_selectors(&report, "/variantKind")?;
        assert!(
            sources.iter().any(|source| {
                string_at(source, "/location/field") == Some("requiredTags")
                    && string_at(source, "/effectiveEncryption") == Some(tag_encryption)
            }),
            "the accepted port requirement is bound to its original XML"
        );
        let route = sources
            .into_iter()
            .find(|source| string_at(source, "/location/field") == Some("patchFile"));
        assert_eq!(
            route.is_some(),
            routed,
            "the original patchFile contributes only when the accepted tag used its explicit patch"
        );
        if let Some(route) = route {
            assert_eq!(
                text_xml_attribute(route, base),
                Some(br#"patchFile="Modifications/Fixture_Paint_Tags.xml""#.as_slice()),
                "the selector points at the original routing attribute"
            );
            assert_eq!(
                string_at(route, "/effectiveEncryption"),
                Some("encrypted"),
                "the routing attribute comes from the encrypted base"
            );
            assert_eq!(
                report
                    .pointer("/fieldOrigins/~1variantKind/containsEncryptedSource")
                    .and_then(Value::as_bool),
                Some(true),
                "the unencrypted tag declaration depends on the encrypted route"
            );
        }
    }
    Ok(())
}

fn text_xml_attribute<'bytes>(source: &Value, bytes: &'bytes [u8]) -> Option<&'bytes [u8]> {
    if string_at(source, "/location/namespace") != Some("textXmlAttribute") {
        return None;
    }
    let start = string_at(source, "/location/indices/0")?
        .parse::<usize>()
        .ok()?;
    let end = string_at(source, "/location/indices/1")?
        .parse::<usize>()
        .ok()?;
    bytes.get(start..end)
}
