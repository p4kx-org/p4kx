// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::DataForge;
use p4kx_testkit::{
    ArchiveBuilder, SCREEN_MFD, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry, TestPort,
    movie_component, screen_component, test_key,
};
use serde_json::Value;

use super::{TestResult, p4kx, screens_sample};

const DCB: &str = "Data/Game2.dcb";
const ENTITY: &str = "libs/foundry/records/entities/scitem/ships/test_screen_mfd";
const SHIP: &str = "TEST_ScreenShip";
const MOVIE: &str = "Data/UI/test.gfx";

fn report(
    path: &Path,
    forge: TestEntry,
    entity: &str,
) -> Result<Value, Box<dyn std::error::Error>> {
    ArchiveBuilder::new()
        .entry(forge)
        .entry(TestEntry::zstd(MOVIE, b"unread movie").encrypted())
        .entry(TestEntry::zstd("Data/UI/Textures/Test/picture.dds", b"unread texture").encrypted())
        .entry(
            TestEntry::zstd(
                "Data/Localization/english/global.ini",
                b"unread translation",
            )
            .encrypted(),
        )
        .write_to(path)?;
    let key = test_key().to_hex();
    let output = p4kx()?
        .args(["--key", &key, "--json", "ui", "screens"])
        .arg(path)
        .arg(entity)
        .output()?;
    assert_eq!(output.status.code(), Some(0), "success: {output:?}");
    Ok(serde_json::from_slice(&output.stdout)?)
}

fn used<'report>(
    report: &'report Value,
    path: &str,
    field: &str,
) -> Result<&'report Value, Box<dyn std::error::Error>> {
    let refs = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(path))
        .and_then(|origin| origin.get("sourceUsesRefs"))
        .and_then(Value::as_array)
        .ok_or("field refs")?;
    report
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("inventory uses")?
        .iter()
        .find(|usage| {
            usage.get("id").is_some_and(|id| refs.contains(id))
                && usage.pointer("/location/field").and_then(Value::as_str) == Some(field)
        })
        .ok_or_else(|| format!("missing {field} use for {path}").into())
}

#[test]
fn an_encrypted_dcb_marks_the_actual_listing_values_and_not_its_entry_name() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bytes = screens_sample().build();
    let forge = DataForge::open(&bytes)?;
    let record_index = forge
        .records_at_path(ENTITY)
        .next()
        .ok_or("source Record")?
        .index()
        .to_string();
    let result = report(
        &dir.path().join("Data.p4k"),
        TestEntry::zstd(DCB, bytes).encrypted(),
        SHIP,
    )?;
    let fields = &result["fieldOrigins"];
    assert_eq!(fields["/archive"]["containsEncryptedSource"], false);
    assert_eq!(fields["/entry"]["containsEncryptedSource"], false);
    assert_eq!(fields["/entity"]["containsEncryptedSource"], true);
    assert_eq!(fields["/brand"]["containsEncryptedSource"], true);
    assert_eq!(fields["/screens/0/width"]["containsEncryptedSource"], true);
    assert_eq!(fields["/screens/0/height"]["containsEncryptedSource"], true);
    assert_eq!(
        fields["/screens/0/port"]["containsEncryptedSource"],
        Value::Null
    );
    let authored = used(&result, "/movies/0/movie", "$value")?;
    assert_eq!(authored["location"]["namespace"], "dataforgeValue");
    assert_eq!(authored["location"]["indices"][0], record_index);
    let relation = used(&result, "/screens/0/width", "$value")?;
    assert_eq!(relation["role"], "selector");
    assert_eq!(relation["location"]["indices"][0], record_index);
    assert_ne!(
        relation["location"]["indices"],
        authored["location"]["indices"]
    );
    let width = used(&result, "/screens/0/width", "size.x")?;
    let height = used(&result, "/screens/0/height", "size.y")?;
    assert_eq!(
        width["location"]["recordIndex"],
        height["location"]["recordIndex"]
    );
    assert_ne!(width["location"], height["location"]);
    Ok(())
}

#[test]
fn authored_asset_names_do_not_prove_the_named_entries_were_consumed() -> TestResult {
    let dir = tempfile::tempdir()?;
    let result = report(
        &dir.path().join("Data.p4k"),
        TestEntry::zstd(DCB, screens_sample().build()),
        SHIP,
    )?;
    assert_eq!(result["movies"][0]["movie"], MOVIE);
    let fields = &result["fieldOrigins"];
    assert_eq!(fields["/entity"]["containsEncryptedSource"], false);
    assert_eq!(
        fields["/movies/0/movie"]["containsEncryptedSource"],
        Value::Null
    );
    assert_eq!(
        used(&result, "/movies/0/movie", "$value")?["effectiveEncryption"],
        "unencrypted"
    );
    let sources = result["provenance"]["sources"]
        .as_array()
        .ok_or("physical sources")?;
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["path"]["value"], DCB);
    Ok(())
}

#[test]
fn direct_widget_assets_and_counts_keep_their_original_scene_sites() -> TestResult {
    let dir = tempfile::tempdir()?;
    let listing = report(
        &dir.path().join("Data.p4k"),
        TestEntry::zstd(DCB, screens_sample().build()),
        "TEST_Screen_MFD",
    )?;
    let path = "/screens/0/assets/textures/0";
    assert_eq!(
        listing.pointer(path).and_then(Value::as_str),
        Some("UI/Textures/Test/picture.dds"),
    );
    assert_eq!(
        listing["fieldOrigins"][path]["containsEncryptedSource"],
        false
    );
    let texture = used(&listing, path, "scene")?;
    assert_eq!(texture["location"]["namespace"], "dataforgeWidgetValue");
    assert_eq!(texture["location"]["indices"][1], "8");
    let elements = listing["screens"][0]["elements"]
        .as_array()
        .ok_or("widget groups")?;
    let index = elements
        .iter()
        .position(|row| row["element"] == "WidgetText")
        .ok_or("text group")?;
    let path = format!("/screens/0/elements/{index}/count");
    let count = used(&listing, &path, "scene")?;
    assert_eq!(count["location"]["namespace"], "dataforgeWidgetType");
    assert_eq!(count["location"]["indices"][1], "6");
    assert_eq!(
        listing["fieldOrigins"][path]["containsEncryptedSource"],
        false
    );
    Ok(())
}

fn default_route_forge(explicit: bool) -> Vec<u8> {
    const ROOT: u128 = 0x48ab_2b09_582c_4289_94f8_949a_37d6_bc89;
    const HOLDER: u128 = 0x60b4_fae5_31d5_4c3d_9fda_35eb_18f1_3199;
    const SCREEN: u128 = 0xb120_5643_ed6b_45be_8c69_bc2e_9830_e7ab;
    let mut root = TestEntity::new(ROOT, "TEST_DefaultRoute", "synthetic/route-root.xml")
        .port(TestPort::new("Outer").default_item(HOLDER));
    if explicit {
        root = root.loadout(TestLoadout::Manual(vec![TestLoadoutEntry::class(
            "oUTER",
            "TEST_RouteHolder",
        )]));
    }
    let holder = TestEntity::new(HOLDER, "TEST_RouteHolder", "synthetic/route-holder.xml")
        .port(TestPort::new("Inner").default_item(SCREEN));
    let screen = TestEntity::new(SCREEN, "TEST_RouteScreen", "synthetic/route-screen.xml")
        .component(screen_component(SCREEN_MFD))
        .component(movie_component(MOVIE));
    screens_sample()
        .record(root.record())
        .record(holder.record())
        .record(screen.record())
        .build()
}

#[test]
fn every_default_port_segment_contributes_its_own_name_and_target() -> TestResult {
    let dir = tempfile::tempdir()?;
    let bytes = default_route_forge(false);
    let forge = DataForge::open(&bytes)?;
    let records: Vec<_> = ["synthetic/route-root", "synthetic/route-holder"]
        .into_iter()
        .map(|path| {
            forge
                .records_at_path(path)
                .next()
                .map(|record| record.index())
        })
        .collect::<Option<_>>()
        .ok_or("source Records")?;
    let listing = report(
        &dir.path().join("Data.p4k"),
        TestEntry::zstd(DCB, bytes),
        "TEST_DefaultRoute",
    )?;
    assert_eq!(listing["screens"][0]["port"], "Outer/Inner");
    let fields = &listing["fieldOrigins"];
    for path in ["/screens/0/port", "/screens/0/width", "/movies/0/movie"] {
        assert_eq!(fields[path]["containsEncryptedSource"], false);
        assert_eq!(fields[path]["sourceSetComplete"], true);
    }
    let selected = fields["/screens/0/port"]["sourceUsesRefs"]
        .as_array()
        .ok_or("port refs")?;
    let uses = listing["provenance"]["uses"]
        .as_array()
        .ok_or("source uses")?;
    assert_eq!(selected.len(), 4);
    for record in records {
        for (suffix, role) in [
            (".Name", "conversion"),
            (".defaultItem.entityClass", "selector"),
        ] {
            assert!(
                uses.iter().any(|usage| {
                    usage.get("id").is_some_and(|id| selected.contains(id))
                        && usage["location"]["recordIndex"] == record
                        && usage["role"] == role
                        && usage
                            .pointer("/location/field")
                            .and_then(Value::as_str)
                            .is_some_and(|field| field.ends_with(suffix))
                }),
                "record {record}, {suffix}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_matching_port_definition_does_not_prove_an_explicit_name() -> TestResult {
    let dir = tempfile::tempdir()?;
    let listing = report(
        &dir.path().join("Data.p4k"),
        TestEntry::zstd(DCB, default_route_forge(true)),
        "TEST_DefaultRoute",
    )?;
    assert_eq!(listing["screens"][0]["port"], "oUTER/Inner");
    let field = &listing["fieldOrigins"]["/screens/0/port"];
    assert_eq!(field["containsEncryptedSource"], Value::Null);
    assert_eq!(field["sourceSetComplete"], false);
    assert_eq!(field["sourceUsesRefs"].as_array().map(Vec::len), Some(2));
    Ok(())
}
