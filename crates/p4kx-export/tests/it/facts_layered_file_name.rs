// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForgeError, Value, ValueError};
use serde_json::{Value as Json, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::super::{BARE, OVERRIDDEN, check_fact, database, same_site};
use super::{Fact, Site, element, field, parameter, site, structure};

const TEMPLATE_PATH: &str = "libs/foundry/records/contracts/contracttemplates/facts.xml";

struct Original {
    template: u32,
    bare_caller: Site,
    parent_caller: Site,
    parent_override: Fact,
}

#[test]
fn unreadable_template_file_name_keeps_the_actual_failed_caller_and_header() -> TestResult {
    let data = unreadable_file_name()?;
    let input = Input::Encrypted;
    let fixture = Fixture::open(&data, input, Vec::new())?;
    let original = read_original(&fixture)?;
    let (_, catalog) = fixture.diagnostic()?;
    let bare = catalog_contracts::entry(&catalog, &health_origin_fixture::guid(BARE), None)?;
    check_unreadable_facts(&catalog, bare, original.bare_caller)?;
    let parent =
        catalog_contracts::entry(&catalog, &health_origin_fixture::guid(OVERRIDDEN), None)?;
    check_fact(
        &catalog,
        parent,
        &original.parent_override,
        input.marker(),
        None,
        &[original.parent_caller],
    )?;
    check_attempts(&catalog, &original)?;
    origin::closure(&catalog)
}

fn unreadable_file_name() -> TestResult<Vec<u8>> {
    let mut data = database().build();
    let mut occurrences = data
        .windows(TEMPLATE_PATH.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == TEMPLATE_PATH.as_bytes()).then_some(index));
    let position = occurrences.next().ok_or("actual Template fileName token")?;
    assert!(
        occurrences.next().is_none(),
        "one physical Template fileName token is being damaged"
    );
    *data
        .get_mut(position)
        .ok_or("actual Template fileName byte")? = 0xff;
    Ok(data)
}

fn read_original(fixture: &Fixture) -> TestResult<Original> {
    let forge = fixture.entry.open()?;
    let template = forge.record(0).ok_or("actual Template Record")?;
    assert_eq!(
        template.type_name()?,
        "ContractTemplate",
        "the original type remains readable when fileName fails"
    );
    let Err(DataForgeError::InvalidString { table, offset }) = template.stored_file_name() else {
        return Err("the exact original fileName must fail its getter".into());
    };
    let generator = forge.record(3).ok_or("actual Generator Record")?;
    let tree = generator.tree()?;
    assert!(
        tree.is_from(&generator),
        "the Generator tree is still readable"
    );
    let handler = element(tree.root(), "generators", 0)?;
    let bare = element(handler, "contracts", 0)?;
    let parent = element(handler, "contracts", 1)?;
    let failed = |contract| -> TestResult<Site> {
        let caller = field(contract, "template")?;
        assert_eq!(
            caller,
            &Value::Error(ValueError::InvalidString { table, offset }),
            "the original Reference decoder reaches the same failed header getter"
        );
        site(&tree, caller)
    };
    let parent_params = structure(field(parent, "paramOverrides")?)?;
    Ok(Original {
        template: template.index(),
        bare_caller: failed(bare)?,
        parent_caller: failed(parent)?,
        parent_override: parameter(
            &tree,
            parent_params,
            1,
            "illegal",
            "Illegal",
            "contract",
            &Value::Bool(true),
        )?,
    })
}

fn check_unreadable_facts(catalog: &Json, bare: &Json, caller: Site) -> TestResult {
    for name in ["canBeShared", "illegal"] {
        let fact = origin::at(bare, &format!("/{name}"))?;
        for part in ["value", "layer"] {
            let cell = origin::cell(fact, part, "unreadable")?;
            assert_eq!(
                cell.get("sourceSetComplete"),
                Some(&json!(false)),
                "{name}/{part} cannot certify a complete Template decision"
            );
            assert!(
                origin::uses(catalog, cell)?
                    .iter()
                    .all(|used| !same_site(used, caller)),
                "{name}/{part} excludes the failed original caller from contributors"
            );
        }
    }
    Ok(())
}

fn check_attempts(catalog: &Json, original: &Original) -> TestResult {
    let failed = origin::rows(catalog, "/exportHealth/failedRecords")?
        .iter()
        .find(|record| {
            record.pointer("/recordIndex/value") == Some(&json!(original.template))
                && record.get("role") == Some(&json!("selectionCandidate"))
                && record.get("stage") == Some(&json!("selection"))
        })
        .ok_or("actual Template selection candidate at its physical Record index")?;
    let path = origin::cell(failed, "recordPath", "unreadable")?;
    assert!(
        origin::uses(catalog, path)?.is_empty(),
        "the unreadable header has no retained path contributors"
    );
    let uses = origin::rows(catalog, "/provenance/uses")?;
    let caller_attempt = uses
        .iter()
        .find(|used| {
            same_site(used, original.bare_caller)
                && used.pointer("/location/field") == Some(&json!("$value"))
                && used.get("role") == Some(&json!("attempted"))
        })
        .ok_or("actual failed Generator slot in the source inventory")?;
    let indices = json!([original.template.to_string()]);
    let header_attempt = uses
        .iter()
        .find(|used| {
            used.pointer("/location/namespace") == Some(&json!("dataforgeRecordHeader"))
                && used.pointer("/location/indices") == Some(&indices)
                && used.pointer("/location/field") == Some(&json!("fileName"))
                && used.get("role") == Some(&json!("attempted"))
        })
        .ok_or("actual attempted Template fileName at the selected Record")?;
    let source = origin::source(catalog, header_attempt)?;
    assert_eq!(
        source.pointer("/path/value"),
        Some(&json!("Data\\Game2.dcb")),
        "the header attempt stays bound to the actual Archive DCB"
    );
    assert_eq!(
        origin::source(catalog, caller_attempt)?,
        source,
        "the exact Generator and header attempts belong to the same Archive DCB"
    );
    assert!(
        uses.iter().all(|used| {
            used.pointer("/location/namespace") != Some(&json!("dataforgeRecordBody"))
                || used.pointer("/location/indices") != Some(&indices)
        }),
        "a failed selection does not claim a Template body read"
    );
    Ok(())
}
