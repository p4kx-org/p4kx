// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestEntry, TestModel, TestSubmesh};
use serde_json::{Value, json};

use crate::model_item_origin_checks::{assert_all_addresses, assert_site, field, marker, uses};
use crate::support::{Install, TestResult};

const MODEL: &str = "Data/cube.cgf";
const BASE: &str = "Data/cube.cgfm";
const LOD_MODEL: &str = "Data/cube_lod1.cgf";
const LOD: &str = "Data/cube_lod1.cgfm";
const RESPONSE_LIMIT: usize = 256 * 1024;
const UNKNOWN_TYPE: u32 = 0x00C0_FFEE;

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
fn chunk_identity_origins_use_page_local_pointers_and_the_actual_file() -> TestResult {
    let cube = TestModel::cube().with_geometry_chunk(UNKNOWN_TYPE, [7; 12]);
    let install = installation(&cube, 2)?;
    let mut server = install.serve(&["--archive", "LIVE"])?;
    let mut args = json!({"path": MODEL, "section": "chunks", "limit": 1});
    let mut seen = 0;
    let mut saw_encrypted_unknown = false;
    let mut usage_labels = std::collections::BTreeSet::new();
    loop {
        let response = server.call("list_model_items", &args)?;
        let reply = &response["structuredContent"];
        let items = reply["items"].as_array().ok_or("chunk rows")?;
        let [row] = items.as_slice() else {
            return Err("expected one page-local chunk".into());
        };
        let (entry, encrypted) = match row["file"].as_str() {
            Some("model") => (MODEL, false),
            Some("geometry") => (BASE, false),
            Some("lod1-model") => (LOD_MODEL, false),
            Some("lod1-geometry") => (LOD, true),
            _ => return Err(format!("unexpected chunk role: {row}").into()),
        };
        let pointer = "items/0/type_id";
        marker(field(reply, pointer)?, encrypted);
        assert_geometry_entry(reply, pointer, "typeId", entry)?;
        assert_chunk_header_origins(reply, encrypted)?;
        assert_chunk_role_origins(reply, entry, encrypted)?;
        usage_labels.insert(row["usage"].as_str().ok_or("reader usage")?.to_owned());
        assert_all_addresses(&response)?;
        seen += 1;
        if row["type_id"] == UNKNOWN_TYPE {
            assert_eq!(row["type"], Value::Null, "unknown type has no name");
            saw_encrypted_unknown |= encrypted;
        }
        if let Some(cursor) = reply.get("next_cursor") {
            args["cursor"] = cursor.clone();
        } else {
            assert_eq!(reply["total"], seen, "all table rows were paged");
            break;
        }
    }
    assert!(
        saw_encrypted_unknown,
        "later pages include an encrypted unknown type"
    );
    assert_eq!(
        usage_labels,
        ["read", "skipped", "unknown"].map(str::to_owned).into()
    );
    Ok(())
}

fn assert_chunk_header_origins(reply: &Value, encrypted: bool) -> TestResult {
    let type_id = "items/0/type_id";
    let kind = "items/0/type";
    let index = "items/0/index";
    let numeric = uses(reply, type_id)?;
    let [word] = numeric.as_slice() else {
        return Err("one original type word".into());
    };
    for path in [kind, index] {
        marker(field(reply, path)?, encrypted);
        let actual = uses(reply, path)?;
        assert_eq!(actual.len(), 2, "{path} has two original contributors");
        assert!(
            actual.iter().all(|site| site["source"] == word["source"]),
            "{path} contributors use the same physical file"
        );
        assert_eq!(
            actual.contains(word),
            path == kind,
            "{path} kind contributor"
        );
    }
    let header = "cryEngineChunkHeader";
    assert_site(reply, kind, header, "signature", &["0", "4"])?;
    assert_site(reply, index, header, "rowCount", &["8", "12"])?;
    assert_site(reply, index, header, "tableOffset", &["12", "16"])?;
    Ok(())
}

fn assert_chunk_role_origins(reply: &Value, entry: &str, encrypted: bool) -> TestResult {
    let numeric = uses(reply, "items/0/type_id")?;
    let [kind] = numeric.as_slice() else {
        return Err("one original type word".into());
    };
    let owner = kind.get("source").ok_or("owning archive entry")?;
    assert_chunk_file_origins(reply, entry, owner, encrypted)?;
    assert_chunk_usage_origins(reply, kind, owner, encrypted)
}

fn assert_chunk_file_origins(
    reply: &Value,
    entry: &str,
    owner: &Value,
    encrypted: bool,
) -> TestResult {
    let sources = reply
        .pointer("/provenance/sources")
        .and_then(Value::as_array)
        .ok_or("physical sources")?;
    let file = "items/0/file";
    let primary = entry == MODEL;
    assert_partial_origin(reply, file, encrypted)?;
    let file_uses = uses(reply, file)?;
    let selectors = file_uses
        .iter()
        .filter(|site| site["role"] == "selector")
        .collect::<Vec<_>>();
    assert_eq!(
        file_uses.len(),
        if primary { 4 } else { 5 },
        "file role contributors"
    );
    assert_eq!(
        selectors.len(),
        if primary { 1 } else { 2 },
        "directory path selectors"
    );
    for path in [Some(entry), (!primary).then_some(MODEL)]
        .into_iter()
        .flatten()
    {
        let id = sources
            .iter()
            .find(|row| row.pointer("/path/value").and_then(Value::as_str) == Some(path))
            .and_then(|row| row.get("id"))
            .ok_or("actual directory path owner")?;
        assert!(
            selectors.iter().any(|site| {
                site.get("source") == Some(id)
                    && site["scope"] == "archiveDirectoryMetadata"
                    && site["location"] == json!({"type": "metadata", "field": "path"})
            }),
            "directory selector for {path}"
        );
    }
    assert!(
        file_uses
            .iter()
            .filter(|site| site["role"] == "payload")
            .all(|site| {
                site.get("source") == Some(owner)
                    && site.pointer("/location/namespace").and_then(Value::as_str)
                        == Some("cryEngineChunkHeader")
            }),
        "table header inputs belong to the chunk's actual file"
    );
    Ok(())
}

fn assert_chunk_usage_origins(
    reply: &Value,
    kind: &Value,
    owner: &Value,
    encrypted: bool,
) -> TestResult {
    let usage = "items/0/usage";
    assert_partial_origin(reply, usage, encrypted)?;
    let usage_uses = uses(reply, usage)?;
    assert_eq!(usage_uses.len(), 2, "observed kind and signature selectors");
    assert!(
        usage_uses.iter().all(|site| {
            site.get("source") == Some(owner)
                && site["role"] == "selector"
                && site["scope"] == "payload"
        }),
        "usage selectors belong to the physical chunk payload"
    );
    assert!(
        usage_uses
            .iter()
            .any(|site| site.get("location") == kind.get("location")),
        "usage includes the original type word"
    );
    assert_site(
        reply,
        usage,
        "cryEngineChunkHeader",
        "signature",
        &["0", "4"],
    )?;
    Ok(())
}

fn assert_partial_origin(reply: &Value, path: &str, encrypted: bool) -> TestResult {
    let origin = field(reply, path)?;
    assert_eq!(
        origin["containsEncryptedSource"].as_bool(),
        encrypted.then_some(true),
        "{path} preserves known encryption with an incomplete contributor set"
    );
    assert_eq!(
        origin["sourceSetComplete"], false,
        "{path} omits unobserved selectors"
    );
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
