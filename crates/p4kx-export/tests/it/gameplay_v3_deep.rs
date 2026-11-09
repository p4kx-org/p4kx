// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{self as export, Compression, Kind};
use p4kx_extraction::{MAX_NESTING_DEPTH, Value as Raw, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};
use crate::gameplay_v3_consumer as consumer;
use crate::gameplay_v3_original as original;
use crate::gameplay_v3_schema as schema;
use crate::gameplay_v3_source as source;
use crate::gameplay_v3_support as support;
use crate::gameplay_v3_values as values;

pub(crate) fn boundary() -> TestResult {
    let depth = MAX_NESTING_DEPTH - 1;
    assert!(
        depth > 32,
        "this oracle crosses the unrelated interpreted-tier depth"
    );
    let database = database(depth);
    original_depth(&database, depth)?;
    let (_folder, archive) = common::open(&database, DcbOrigin::Plain)?;
    let options = common::options();
    let built = export::build_catalog(&archive, &options)?;
    emitted(built.json(), depth)?;
    let plain: Value = serde_json::from_slice(built.json())?;
    accepted(&plain, &database, depth)?;
    let (summary, compressed) =
        export::write_catalog(&archive, &options, Compression::ZSTD, || Ok(Vec::new()))?;
    let bytes = zstd::stream::decode_all(compressed.as_slice())?;
    let packed: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        packed, plain,
        "the entire legal-depth plain and decoded zstd catalogs agree"
    );
    accepted(&packed, &database, depth)?;
    assert_eq!(
        summary
            .kind(Kind::GameplayProperties)
            .map(|kind| kind.count),
        Some(1),
        "deep nodes do not create another physical definition count"
    );
    beyond_boundary()
}

fn database(sequences: usize) -> DataForgeBuilder {
    let mut display = values::scale(TestValue::Single(-0.0));
    for _ in 0..sequences {
        display = values::sequence(vec![display]);
    }
    schema::schema().record(values::record(values::FIRST, "Primary", display))
}

fn original_depth(database: &DataForgeBuilder, sequences: usize) -> TestResult {
    source::with_tree(database, 0, |tree| {
        let mut current = source::structure(tree.root().field("displayTransformation"))?;
        for _ in 0..sequences {
            assert_eq!(
                current.type_name(),
                schema::SEQUENCE,
                "original admitted sequence depth"
            );
            current = source::structure(source::array(current.field("transformations"))?.first())?;
        }
        assert_eq!(
            current.type_name(),
            schema::SCALE,
            "the original deepest leaf is materialized"
        );
        let Some(Raw::Single(value)) = current.field("scale") else {
            return Err("original deepest Single".into());
        };
        assert_eq!(
            value.to_bits(),
            0x8000_0000,
            "the original deepest token has signed-zero bits"
        );
        Ok(())
    })
}

fn emitted(bytes: &[u8], depth: usize) -> TestResult {
    let text = std::str::from_utf8(bytes)?;
    assert!(
        text.contains("\"gameplayProperties\":{\"availability\":\"available\",\"rows\":["),
        "the normal deep export contains the canonical table"
    );
    assert_eq!(
        text.matches(&format!("\"value\":\"{}\"", schema::SEQUENCE))
            .count(),
        depth,
        "every original recursive source-type datum is serialized"
    );
    assert!(
        text.contains("\"rawBits\":\"80000000\""),
        "the actual deepest IEEE leaf is emitted"
    );
    Ok(())
}

fn accepted(catalog: &Value, database: &DataForgeBuilder, depth: usize) -> TestResult {
    let schema = export::catalog_schema();
    let validator = jsonschema::validator_for(&schema)?;
    assert!(
        validator.is_valid(catalog),
        "the full legal-source-depth catalog satisfies its actual schema"
    );
    assert_eq!(
        consumer::rows(catalog, Kind::GameplayProperties)?.len(),
        1,
        "the actual canonical consumer accepts the whole deep catalog"
    );
    let row = support::row(catalog, 0)?;
    let nodes = support::at(row, "/displayTransformation/value/nodes")?
        .as_array()
        .ok_or("deep occurrence vector")?;
    assert_eq!(
        nodes.len(),
        MAX_NESTING_DEPTH,
        "all original owned occurrences survive the flat wire"
    );
    let leaf = nodes.get(depth).ok_or("actual deepest leaf occurrence")?;
    assert_eq!(
        support::at(leaf, "/payload/value/scale/value/rawBits")?,
        &json!("80000000"),
        "default Value parsing retains the deepest original bits"
    );
    original::check(catalog, database, 0)
}

fn beyond_boundary() -> TestResult {
    let database = database(MAX_NESTING_DEPTH);
    original_limit_error(&database)?;
    let (_, catalog) = support::diagnostic(&database)?;
    let row = support::row(&catalog, 0)?;
    let nodes = support::at(row, "/displayTransformation/value/nodes")?
        .as_array()
        .ok_or("admitted prefix")?;
    assert_eq!(
        nodes.len(),
        MAX_NESTING_DEPTH,
        "a failed owned slot does not invent another node"
    );
    let last = nodes.last().ok_or("last admitted sequence")?;
    support::state(
        support::at(last, "/payload/value/transformations/value/0")?,
        "unreadable",
    )?;
    let path = format!(
        "displayTransformation{}",
        ".transformations[0]".repeat(MAX_NESTING_DEPTH)
    );
    support::diagnosis(&catalog, "unreadableSource", 0, &path, schema::SEQUENCE)?;
    original::check(&catalog, &database, 0)?;
    support::refused(&database, None)
}

fn original_limit_error(database: &DataForgeBuilder) -> TestResult {
    source::with_tree(database, 0, |tree| {
        let mut current = source::structure(tree.root().field("displayTransformation"))?;
        for _ in 1..MAX_NESTING_DEPTH {
            current = source::structure(source::array(current.field("transformations"))?.first())?;
        }
        assert_eq!(
            current.type_name(),
            schema::SEQUENCE,
            "the final admitted original node is still a sequence"
        );
        let leaf = source::array(current.field("transformations"))?.first();
        assert!(
            matches!(leaf, Some(Raw::Error(ValueError::TooDeep { limit })) if *limit == MAX_NESTING_DEPTH),
            "the first beyond-limit original pointer is a genuine format depth error"
        );
        Ok(())
    })
}
