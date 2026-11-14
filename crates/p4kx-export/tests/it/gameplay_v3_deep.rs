// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{self as export, Compression, Kind};
use p4kx_extraction::Value as Raw;
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
    for depth in [63, 64] {
        let database = database(depth);
        original_depth(&database, depth)?;
        complete(&database, depth)?;
    }
    Ok(())
}

#[test]
fn deeply_nested_gameplay_transformations_keep_their_original_order_and_bits() -> TestResult {
    let depth = 257;
    let database = database(depth);
    original_depth(&database, depth)?;
    complete(&database, depth)
}

fn complete(database: &DataForgeBuilder, depth: usize) -> TestResult {
    let (_folder, archive) = common::open(database, DcbOrigin::Plain)?;
    let options = common::options();
    let built = export::build_catalog(&archive, &options)?;
    emitted(built.json(), depth)?;
    let plain: Value = serde_json::from_slice(built.json())?;
    accepted(&plain, database, depth)?;
    let (summary, compressed) =
        export::write_catalog(&archive, &options, Compression::ZSTD, || Ok(Vec::new()))?;
    let bytes = zstd::stream::decode_all(compressed.as_slice())?;
    let packed: Value = serde_json::from_slice(&bytes)?;
    assert_eq!(
        packed, plain,
        "the whole complete plain and decoded zstd catalogs agree"
    );
    accepted(&packed, database, depth)?;
    assert_eq!(
        summary
            .kind(Kind::GameplayProperties)
            .map(|kind| kind.count),
        Some(1),
        "owned nodes do not create another physical definition count"
    );
    Ok(())
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
        depth + 1,
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
