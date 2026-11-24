// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestEntry, TestModel, TestSubmesh};
use serde_json::{Value, json};

use crate::model_item_origin_checks::{assert_all_addresses, field, marker, uses};
use crate::support::{Install, TestResult};

const MODEL: &str = "Data/cube.cgf";
const BASE: &str = "Data/cube.cgfm";
const LOD: &str = "Data/cube_lod1.cgfm";
const RESPONSE_LIMIT: usize = 256 * 1024;

#[test]
fn mesh_page_origins_follow_the_original_lod_with_page_local_addresses() -> TestResult {
    let cube = TestModel::cube();
    let install = installation(&cube, 2)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut args = json!({"path": MODEL, "section": "meshes", "limit": 1});
    for (level, entry, encrypted) in [(0, BASE, false), (1, LOD, true)] {
        let response = server.call("list_model_items", &args)?;
        let reply = &response["structuredContent"];
        assert_eq!(reply["total"], 2, "both original mesh rows are counted");
        assert_eq!(
            reply.pointer("/items/0/lod"),
            Some(&json!(level)),
            "original LOD order"
        );
        assert_eq!(
            reply["items"].as_array().map(Vec::len),
            Some(1),
            "one page-local row"
        );
        let submesh = "items/0/submeshes/0";
        for (name, label) in [("material_id", "materialId"), ("index_count", "indexCount")] {
            let path = format!("{submesh}/{name}");
            marker(field(reply, &path)?, encrypted);
            assert_geometry_entry(reply, &path, label, entry)?;
        }
        let origin = field(reply, &format!("{submesh}/first_index"))?;
        assert_eq!(
            origin,
            &json!({"containsEncryptedSource": null, "sourceSetComplete": false, "sourceUsesRefs": []}),
            "the rebuilt first index stays unknown"
        );
        assert_all_addresses(&response)?;
        if level == 0 {
            args["cursor"] = reply.get("next_cursor").ok_or("next mesh page")?.clone();
        } else {
            assert!(
                reply.get("next_cursor").is_none(),
                "the original cursor ends at the last LOD"
            );
        }
    }
    Ok(())
}

#[test]
fn mesh_origin_evidence_counts_toward_the_existing_page_budget() -> TestResult {
    const LEVELS: usize = 4;
    const ROWS_PER_MESH: u32 = 160;
    const TRIANGLE: [u32; 3] = [0, 1, 2];
    const INDICES_PER_TRIANGLE: u32 = 3;
    let submeshes: Vec<_> = (0..ROWS_PER_MESH)
        .map(|row| TestSubmesh {
            material_id: 1,
            node: 1,
            first_index: row * INDICES_PER_TRIANGLE,
            index_count: INDICES_PER_TRIANGLE,
            first_vertex: 0,
            vertex_count: 3,
        })
        .collect();
    let cube = TestModel::cube()
        .with_indices(TRIANGLE.repeat(submeshes.len()))
        .with_submeshes(submeshes);
    let install = installation(&cube, LEVELS)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut arguments = json!({"path": MODEL, "section": "meshes", "limit": 1000});
    let mut next_level = 0;
    loop {
        let response = server.call("list_model_items", &arguments)?;
        let reply = &response["structuredContent"];
        assert_eq!(reply["total"], LEVELS);
        assert!(
            serde_json::to_vec(reply)?.len() <= RESPONSE_LIMIT,
            "source evidence shares the response budget"
        );
        let rows = reply
            .get("items")
            .and_then(Value::as_array)
            .ok_or("mesh rows")?;
        assert!(!rows.is_empty(), "the cursor advances");
        for (local, row) in rows.iter().enumerate() {
            assert_eq!(
                row["lod"], next_level,
                "each LOD must appear once and in order"
            );
            let output = format!("items/{local}/submeshes/{}/index_count", ROWS_PER_MESH - 1);
            marker(field(reply, &output)?, next_level == 1);
            next_level += 1;
        }
        assert_all_addresses(&response)?;
        if let Some(cursor) = reply.get("next_cursor") {
            assert!(next_level < LEVELS, "a cursor points to a remaining mesh");
            arguments["cursor"] = cursor.clone();
        } else {
            assert!(LEVELS > rows.len(), "origin evidence forces pagination");
            break;
        }
    }
    assert_eq!(next_level, LEVELS, "all original LODs were emitted");
    Ok(())
}

fn installation(cube: &TestModel, levels: usize) -> TestResult<Install> {
    let mut model = cube.clone();
    for _ in 1..levels {
        model = model.with_lod(cube.clone());
    }
    let mut entries = vec![
        TestEntry::zstd(MODEL, model.cgf()),
        TestEntry::zstd(BASE, model.cgfm()),
    ];
    for level in 1..levels {
        let cgf = model.lod_cgf(level).ok_or("LOD definition")?;
        let cgfm = model.lod_cgfm(level).ok_or("LOD geometry")?;
        entries.push(TestEntry::zstd(format!("Data/cube_lod{level}.cgf"), cgf));
        let entry = TestEntry::zstd(format!("Data/cube_lod{level}.cgfm"), cgfm);
        entries.push(if level == 1 { entry.encrypted() } else { entry });
    }
    Install::new(entries)
}

fn assert_geometry_entry(reply: &Value, output: &str, label: &str, entry: &str) -> TestResult {
    let sites = uses(reply, output)?;
    let [site] = sites.as_slice() else {
        return Err(format!("{output} requires one source word").into());
    };
    assert_eq!(
        site.get("role").and_then(Value::as_str),
        Some("payload"),
        "{output}"
    );
    assert_eq!(
        site.pointer("/location/field").and_then(Value::as_str),
        Some(label),
        "{output}"
    );
    assert_eq!(
        site.pointer("/location/namespace").and_then(Value::as_str),
        Some("model-byte-range"),
        "{output}"
    );
    let sources = reply
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let source = sources
        .iter()
        .find(|row| row.get("id") == site.get("source"))
        .ok_or("selected geometry source")?;
    assert_eq!(
        source.pointer("/path/value").and_then(Value::as_str),
        Some(entry),
        "{output}"
    );
    Ok(())
}
