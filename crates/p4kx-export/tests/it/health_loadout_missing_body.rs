// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    ArchiveFiles, Entities, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutResolver,
};
use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_extraction::Value as ForgeValue;
use serde_json::{Value, json};

use crate::health_origin_assertions as check;
use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};
use crate::health_origin_loadout_source as source;

const REQUEST: &str = "synthetic/shared.xml";

#[test]
fn missing_loadout_examples_keep_each_original_request_and_its_root_type() -> TestResult {
    let input = Fixture::open(&source::database(), Input::Encrypted, Vec::new())?;
    original(&input)?;
    let (_, catalog) = input.diagnostic()?;
    let diagnostics = check::rows(&catalog, "/exportHealth/diagnostics")?;
    let diagnostic = diagnostics
        .iter()
        .find(|row| {
            row.get("code") == Some(&json!("danglingReference"))
                && row.get("warningCode") == Some(&json!("xml_loadout"))
                && row.get("kind") == Some(&json!("loadouts"))
        })
        .ok_or("the actual missing XML Loadout diagnostic is absent")?;
    assert_eq!(
        diagnostic.get("count"),
        Some(&json!(2)),
        "both original callers remain accounted"
    );
    let examples = check::rows(diagnostic, "/examples")?;
    assert_eq!(
        examples.len(),
        2,
        "the equal file requests keep distinct physical caller examples"
    );
    for (index, example) in examples.iter().enumerate() {
        let index = u32::try_from(index)?;
        check::present(example, "recordIndex", &json!(index), Some(true))?;
        let field = check::present_text(&catalog, example, "field", &json!(REQUEST), Some(true))?;
        request_origin(&catalog, field, None)?;
        let kind = check::present_text(
            &catalog,
            example,
            "sourceType",
            &json!("EntityClassDefinition"),
            Some(true),
        )?;
        check::header(
            &catalog,
            kind,
            index,
            "sourceType",
            "selector",
            "archiveEntry",
        )?;
        assert_eq!(
            example.pointer("/targetGuid/state"),
            Some(&json!("absent")),
            "an absent file has no fabricated target GUID"
        );
    }
    check::closure(&catalog)
}

fn original(input: &Fixture) -> TestResult {
    let forge = input.entry.open()?;
    assert_eq!(
        forge.records().count(),
        2,
        "two original physical caller Records"
    );
    let files = ArchiveFiles::new(&input.archive);
    let observed = files.read_observed(REQUEST);
    assert!(
        matches!(observed.outcome(), Err(LoadoutFileError::NotFound)),
        "the actual Archive lookup is missing"
    );
    assert!(
        observed.source().entry_origin().is_none(),
        "no target Entry is fabricated"
    );
    assert!(
        observed
            .source()
            .archive_input_origin()
            .is_some_and(|source| source.is_same_input(input.archive.input_origin())),
        "the genuine missing operation belongs to this actual Archive"
    );
    let mut resolver =
        LoadoutResolver::with_entities(Arc::new(Entities::new(&forge))).with_files(&files);
    for index in [0, 1] {
        let record = forge
            .record(index)
            .ok_or("the actual caller Record is absent")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the original canonical tree belongs to that physical caller"
        );
        original_path(&tree)?;
        let node = resolver.resolve(&record)?;
        assert!(
            node.issues().iter().any(|issue| matches!(issue,
            LoadoutError::MissingXmlLoadout { path } if path.to_string() == REQUEST)),
            "the real resolver retains the actual missing authored request"
        );
    }
    Ok(())
}

fn original_path(tree: &p4kx_extraction::RecordTree<'_>) -> TestResult {
    let Some(ForgeValue::Array(components)) = tree.field("Components") else {
        return Err("the genuine original Components array is absent".into());
    };
    let component = components
        .iter()
        .find_map(|value| match value {
            ForgeValue::Struct(value)
                if value.type_name() == "SEntityComponentDefaultLoadoutParams" =>
            {
                Some(value)
            }
            _ => None,
        })
        .ok_or("the genuine original loadout component is absent")?;
    let Some(ForgeValue::Struct(loadout)) = component.field("loadout") else {
        return Err("the genuine original Loadout is absent".into());
    };
    assert_eq!(
        loadout.type_name(),
        "SItemPortLoadoutXMLParams",
        "the actual XML-loadout source variant"
    );
    assert!(
        matches!(loadout.field("loadoutPath"), Some(ForgeValue::String(value)) if *value == REQUEST),
        "the original scalar stores the complete unchanged authored request"
    );
    Ok(())
}

fn request_origin(catalog: &Value, cell: &Value, expected: Option<u32>) -> TestResult {
    let uses = check::uses(catalog, cell)?;
    let mut payloads = uses
        .iter()
        .filter(|contribution| contribution.get("role") == Some(&json!("payload")));
    let contribution = payloads
        .next()
        .ok_or("the original path payload is absent")?;
    assert!(
        payloads.next().is_none(),
        "exactly the original authored path datum is payload"
    );
    assert_eq!(
        contribution.pointer("/location/namespace"),
        Some(&json!("dataforgeValue")),
        "the actual full-record namespace"
    );
    if let Some(index) = expected {
        assert_eq!(
            contribution.pointer("/location/indices/0"),
            Some(&json!(index.to_string())),
            "the only selected original caller supplies its unchanged path payload"
        );
    }
    assert_eq!(
        contribution.pointer("/location/field"),
        Some(&json!("$value")),
        "the actual original string datum"
    );
    assert_eq!(
        check::source(catalog, contribution)?.pointer("/path/value"),
        Some(&json!("Data\\Game2.dcb")),
        "the caller DCB supplies the path rather than an invented target Entry"
    );
    assert!(
        uses.iter().all(
            |contribution| contribution.get("role") == Some(&json!("payload"))
                || contribution.get("role") == Some(&json!("selector"))
        ),
        "copying the request adds no conversion or unrelated payload"
    );
    Ok(())
}

#[test]
fn each_filtered_original_caller_supplies_its_exact_missing_path_payload() -> TestResult {
    let input = Fixture::open(&source::database(), Input::Encrypted, Vec::new())?;
    original(&input)?;
    for index in [0, 1] {
        let filter = Filter::new(&format!("*health{index}*"));
        let forge = input.entry.open()?;
        let selected: Vec<_> = forge
            .records()
            .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
            .map(|record| record.index())
            .collect();
        assert_eq!(
            selected,
            [index],
            "the original path filter selects exactly this physical caller"
        );
        let options = CatalogOptions {
            filter: Some(&filter),
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..fixture::options()
        };
        let built = build_catalog_from(&input.archive, &input.entry, &options)?;
        let catalog: Value = serde_json::from_slice(built.json())?;
        filtered_example(&catalog, index)?;
    }
    Ok(())
}

fn filtered_example(catalog: &Value, index: u32) -> TestResult {
    let diagnostic = check::rows(catalog, "/exportHealth/diagnostics")?
        .iter()
        .find(|row| {
            row.get("code") == Some(&json!("danglingReference"))
                && row.get("warningCode") == Some(&json!("xml_loadout"))
                && row.get("kind") == Some(&json!("loadouts"))
        })
        .ok_or("the actual single selected missing issue")?;
    assert_eq!(
        diagnostic.get("count"),
        Some(&json!(1)),
        "one genuinely selected original caller"
    );
    let examples = check::rows(diagnostic, "/examples")?;
    assert_eq!(
        examples.len(),
        1,
        "no unselected caller example is fabricated"
    );
    let example = examples.first().ok_or("the selected original example")?;
    check::present(example, "recordIndex", &json!(index), Some(true))?;
    let field = check::present_text(catalog, example, "field", &json!(REQUEST), Some(true))?;
    request_origin(catalog, field, Some(index))?;
    let kind = check::present_text(
        catalog,
        example,
        "sourceType",
        &json!("EntityClassDefinition"),
        Some(true),
    )?;
    check::header(
        catalog,
        kind,
        index,
        "sourceType",
        "selector",
        "archiveEntry",
    )?;
    assert_eq!(
        example.pointer("/targetGuid/state"),
        Some(&json!("absent")),
        "no invented target GUID"
    );
    check::closure(catalog)?;
    Ok(())
}
