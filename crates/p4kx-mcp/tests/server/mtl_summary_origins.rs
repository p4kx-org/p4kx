// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestContainer, TestEntry, TestMtl, TestMtlMaterial};
use serde_json::{Value, json};

use crate::support::{Install, TestResult};

const MATERIAL: &str = "Data/probe.mtl";
const PACK: &str = "Data/pack.socpak";
const NESTED: &str = "Data/pack.socpak::materials/PROBE.mtl";
const RESPONSE_LIMIT: usize = 256 * 1024;

const XML: &str = r#"<Material xmlns:q="urn:fixture" q:DefaultPalette="shadow" DefaultPalette="winner">
    <Other/>
    <SubMaterials>
        <MatRef/>
        <Material q:Name="shadow" Name="chosen" MtlFlags="1024" Opacity="invalid" Diffuse="0.5,0.25,0.125">
            <Other/>
            <Textures>
                <Other/>
                <Texture Map="ignored" File=""/>
                <Texture q:File="shadow.dds" Map="chosen" File="winning.dds"/>
                <Texture File="second.dds"/>
            </Textures>
            <PublicParams Same="first" Other="other"/>
            <PublicParams Same="second"/>
            <MatLayers>
                <Other/>
                <Layer Name="layer" path="winner.mtl" Path="shadow.mtl" palettetint="99" PaletteTint="1"/>
                <Layer PaletteTint="2"/>
            </MatLayers>
        </Material>
        <Material Name="next"/>
    </SubMaterials>
    <SubMaterials><Material Name="shadow"/></SubMaterials>
</Material>"#;

#[test]
fn nested_material_fields_use_the_admitted_xml_slots_and_the_selected_archive_entry() -> TestResult
{
    let container = TestContainer::new()
        .entry(TestEntry::stored("materials/probe.mtl", XML))
        .entry(TestEntry::stored(
            "materials/PROBE.mtl",
            "<Material Name='shadow'/>",
        ))
        .build();
    let install = Install::new(vec![TestEntry::deflated(PACK, container).encrypted()])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let first = server.call("mtl_summary", &json!({"path": NESTED, "limit": 1}))?;
    let fallback = &first["structuredContent"];
    assert_fallback(fallback)?;
    assert_all_addresses(&first)?;

    let second = server.call(
        "mtl_summary",
        &json!({"path": NESTED, "limit": 1, "cursor": fallback["next_cursor"]}),
    )?;
    let reply = &second["structuredContent"];
    assert_eq!(reply["path"], "Data/pack.socpak::materials\\probe.mtl");
    assert_eq!(reply["materials"][0]["id"], 1);
    assert_eq!(reply["materials"][0]["name"], "chosen");
    assert_eq!(reply["materials"][0]["no_draw"], true);
    assert_eq!(reply["materials"][0]["opacity"], 1.0);
    assert_eq!(reply["materials"][0]["textures"][0]["file"], "winning.dds");
    assert_eq!(reply["materials"][0]["textures"][1]["map"], "");
    assert_eq!(reply["materials"][0]["public_params"][0]["value"], "first");
    assert_eq!(reply["materials"][0]["public_params"][2]["value"], "second");
    assert_eq!(reply["materials"][0]["layers"][0]["path"], "winner.mtl");
    assert_eq!(reply["materials"][0]["layers"][0]["palette_tint"], "99");
    assert_eq!(reply["materials"][0]["palette"], "primary");
    assert_eq!(field(reply, "path")?["containsEncryptedSource"], true);
    assert_eq!(uses(reply, "path")?.len(), 2);
    assert_eq!(field(reply, "archive")?["containsEncryptedSource"], false);
    assert_eq!(
        field(reply, "materials/0/name")?["containsEncryptedSource"],
        true
    );
    assert_eq!(field(reply, "materials/0/name")?["sourceSetComplete"], true);
    assert_nested_sites(reply)?;
    assert_eq!(
        reply["provenance"]["sources"].as_array().map(Vec::len),
        Some(2)
    );
    assert!(
        uses(reply, "materials/0/palette")?.iter().all(
            |site| site.pointer("/location/indices") != Some(&json!(["1", "1", "4", "2", "0"]))
        ),
        "a later layer's tint cannot contribute to the already selected palette"
    );
    assert!(field(reply, "materials/0/layers/1/path").is_err());
    assert_all_addresses(&second)
}

#[test]
fn a_plain_cryxml_winner_does_not_inherit_an_encrypted_duplicate_or_a_named_dependency()
-> TestResult {
    let library = TestMtl::single(
        &TestMtlMaterial::new("plain")
            .texture("TexSlot1", "selected.dds")
            .public_param("Same", "first")
            .public_param("Same", "second"),
    )
    .cryxml();
    let install = Install::new(vec![
        TestEntry::stored(MATERIAL, library),
        TestEntry::stored("data/PROBE.MTL", "<Material Name='shadow'/>").encrypted(),
        TestEntry::stored("selected.dds", b"unread fixture".to_vec()).encrypted(),
    ])?;
    let mut server = install.serve_without_key(&["--archive", "LIVE"])?;
    let response = server.call("mtl_summary", &json!({"path": "data/PROBE.MTL"}))?;
    let reply = &response["structuredContent"];
    assert_eq!(reply["materials"][0]["name"], "plain");
    assert_eq!(
        reply["provenance"]["sources"].as_array().map(Vec::len),
        Some(1)
    );
    assert_eq!(
        field(reply, "materials/0/textures/0/file")?["containsEncryptedSource"],
        false
    );
    assert_eq!(
        field(reply, "materials/0/textures/0/file")?["sourceSetComplete"],
        true
    );
    assert_eq!(reply["materials"][0]["public_params"][0]["value"], "first");
    assert_eq!(reply["materials"][0]["public_params"][1]["value"], "second");
    assert_site(
        reply,
        "materials/0/public_params/0/value",
        "materialXmlAttribute",
        "Same",
        &["1", "0"],
    )?;
    assert_site(
        reply,
        "materials/0/public_params/1/value",
        "materialXmlAttribute",
        "Same",
        &["1", "1"],
    )?;
    let tools = server.tools()?;
    let schema = &tools
        .iter()
        .find(|tool| tool["name"] == "mtl_summary")
        .ok_or("material tool")?["outputSchema"];
    let required = schema["required"]
        .as_array()
        .ok_or("required result fields")?;
    assert!(required.contains(&json!("fieldOrigins")));
    assert!(required.contains(&json!("provenance")));
    assert_all_addresses(&response)
}

#[test]
fn material_provenance_is_paged_with_its_rows_under_the_existing_cap() -> TestResult {
    let total = 140;
    let rows = (0..total)
        .map(|id| {
            TestMtlMaterial::new(format!("material {id}"))
                .texture("TexSlot1", "selected.dds")
                .public_param("value", "written")
        })
        .collect::<Vec<_>>();
    let install = Install::new(vec![TestEntry::stored(
        MATERIAL,
        TestMtl::multi(&rows).cryxml(),
    )])?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut arguments = json!({"path": MATERIAL, "limit": 1000});
    let mut next = 0;
    loop {
        let response = server.call("mtl_summary", &arguments)?;
        let reply = &response["structuredContent"];
        let start = next;
        assert_eq!(reply["total"], total);
        assert!(serde_json::to_vec(reply)?.len() <= RESPONSE_LIMIT);
        let rows = reply["materials"].as_array().ok_or("material rows")?;
        assert!(!rows.is_empty(), "each page advances");
        for (position, row) in rows.iter().enumerate() {
            assert_eq!(row["id"], next);
            assert_site(
                reply,
                &format!("materials/{position}/name"),
                "materialXmlAttribute",
                "Name",
                &["0", &next.to_string(), "0"],
            )?;
            next += 1;
        }
        assert!(
            reply["provenance"]["uses"]
                .as_array()
                .ok_or("source uses")?
                .iter()
                .all(|site| {
                    site.pointer("/location/indices/1")
                        .and_then(Value::as_str)
                        .and_then(|ordinal| ordinal.parse::<usize>().ok())
                        .is_none_or(|ordinal| (start..next).contains(&ordinal))
                }),
            "unreturned material rows have no proof in this response"
        );
        assert_all_addresses(&response)?;
        if let Some(cursor) = reply.get("next_cursor") {
            arguments["cursor"] = cursor.clone();
        } else {
            assert!(start > 0, "origin metadata forces pagination");
            break;
        }
    }
    assert_eq!(next, total);
    Ok(())
}

fn assert_fallback(fallback: &Value) -> TestResult {
    for (path, expected) in [
        ("/total", json!(3)),
        ("/unknown_entries", json!(1)),
        ("/default_palette", json!("winner")),
        ("/materials/0/id", json!(0)),
        ("/materials/0/name", json!("")),
        ("/materials/0/textures", json!([])),
    ] {
        assert_eq!(
            fallback.pointer(path),
            Some(&expected),
            "expected value at {path}"
        );
    }
    assert_site(
        fallback,
        "materials/0/textures",
        "materialXmlElement",
        "engineDefaultSlot",
        &["1", "0"],
    )?;
    assert_site(
        fallback,
        "default_palette",
        "materialXmlAttribute",
        "DefaultPalette",
        &["1"],
    )?;
    assert_site(
        fallback,
        "total",
        "materialXmlElement",
        "materialSlots",
        &["1"],
    )?;
    Ok(())
}

fn assert_nested_sites(reply: &Value) -> TestResult {
    for (output, source, ordinal) in [
        ("materials/0/name", "Name", "1"),
        ("materials/0/no_draw", "MtlFlags", "2"),
        ("materials/0/opacity", "Opacity", "3"),
        ("materials/0/diffuse/2", "Diffuse", "4"),
    ] {
        assert_site(
            reply,
            output,
            "materialXmlAttribute",
            source,
            &["1", "1", ordinal],
        )?;
    }
    assert_site(
        reply,
        "materials/0/textures/0/file",
        "materialXmlAttribute",
        "File",
        &["1", "1", "1", "2", "2"],
    )?;
    assert_site(
        reply,
        "materials/0/textures/1/map",
        "materialXmlElement",
        "Map",
        &["1", "1", "1", "3"],
    )?;
    assert_site(
        reply,
        "materials/0/public_params/2/value",
        "materialXmlAttribute",
        "Same",
        &["1", "1", "3", "0"],
    )?;
    assert_site(
        reply,
        "materials/0/layers/0/path",
        "materialXmlAttribute",
        "path",
        &["1", "1", "4", "1", "1"],
    )?;
    assert_site(
        reply,
        "materials/0/layers/0/palette_tint",
        "materialXmlAttribute",
        "palettetint",
        &["1", "1", "4", "1", "3"],
    )?;
    assert_site(
        reply,
        "materials/0/palette",
        "materialXmlAttribute",
        "PaletteTint",
        &["1", "1", "4", "1", "4"],
    )?;
    Ok(())
}

fn assert_all_addresses(response: &Value) -> TestResult {
    let fields = response
        .pointer("/structuredContent/fieldOrigins")
        .and_then(Value::as_object)
        .ok_or("field origins")?;
    for pointer in fields.keys() {
        assert!(
            response.pointer(pointer).is_some(),
            "nonexistent output field {pointer}"
        );
    }
    Ok(())
}

fn assert_site(
    reply: &Value,
    pointer: &str,
    namespace: &str,
    field: &str,
    indices: &[&str],
) -> TestResult {
    assert!(
        uses(reply, pointer)?.iter().any(|site| {
            site.pointer("/location/namespace").and_then(Value::as_str) == Some(namespace)
                && site.pointer("/location/field").and_then(Value::as_str) == Some(field)
                && site.pointer("/location/indices") == Some(&json!(indices))
        }),
        "{pointer} must retain {field} at original {namespace} {indices:?}"
    );
    Ok(())
}

fn field<'a>(reply: &'a Value, pointer: &str) -> TestResult<&'a Value> {
    reply
        .get("fieldOrigins")
        .and_then(|origins| origins.get(format!("/structuredContent/{pointer}")))
        .ok_or_else(|| format!("missing field origin at {pointer}").into())
}

fn uses<'a>(reply: &'a Value, pointer: &str) -> TestResult<Vec<&'a Value>> {
    let sites = reply
        .pointer("/provenance/uses")
        .and_then(Value::as_array)
        .ok_or("source inventory")?;
    field(reply, pointer)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("source uses")?
        .iter()
        .map(|reference| {
            sites
                .iter()
                .find(|site| site.get("id") == Some(reference))
                .ok_or_else(|| format!("unresolved use at {pointer}").into())
        })
        .collect()
}
