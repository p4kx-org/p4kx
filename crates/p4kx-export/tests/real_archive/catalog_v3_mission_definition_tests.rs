// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::Kind;
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as fixture, TestResult};
use crate::real_archive_verification;

const OWNERS: [(Kind, u128, &str, &str); 2] = [
    (
        Kind::ContractTemplates,
        0x9e01,
        "ContractTemplate",
        "libs/foundry/records/contracts/contracttemplates/Acceptance.xml",
    ),
    (
        Kind::MissionBrokerEntries,
        0x9e02,
        "MissionBrokerEntry",
        "libs/foundry/records/missionbroker/Acceptance.xml",
    ),
];

fn database() -> DataForgeBuilder {
    OWNERS.iter().fold(
        fixture::database(),
        |builder, &(_, guid, source_type, file)| {
            builder
                .structure(TestStruct::new(source_type))
                .record(TestRecord::new(
                    guid,
                    source_type,
                    format!("{source_type}.Acceptance"),
                    file,
                ))
        },
    )
}

#[test]
fn mission_definition_inventory_uses_original_physical_headers() -> TestResult {
    let database = database();
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let original = original_owners(&forge)?;
    let (_, catalog) = fixture::build(&database)?;
    real_archive_verification::check_unique_guids(&catalog)?;
    for (kind, index) in original {
        let [row] = reader::rows(&catalog, kind)?.as_slice() else {
            return Err(format!("one actual canonical {kind} definition").into());
        };
        let record = forge.record(index).ok_or("actual original mission owner")?;
        assert_eq!(reader::identity(row, kind)?, record.guid().to_string());
        assert_eq!(reader::field(row, kind, "sourceType")?, record.type_name()?);
        assert_eq!(reader::field(row, kind, "recordIndex")?, &json!(index));
        assert_eq!(
            reader::field(row, kind, "fileName")?,
            record.stored_file_name()?
        );
        legacy_distraction(&catalog, kind, row)?;
    }
    Ok(())
}

#[test]
fn canonical_contract_rows_are_required_even_when_the_population_is_empty() -> TestResult {
    let (_, mut catalog) = fixture::build(&database())?;
    assert!(
        reader::rows(&catalog, Kind::Contracts)?.is_empty(),
        "the actual producer has no Generator contracts in this fixture"
    );
    catalog.as_object_mut().ok_or("catalog object")?.insert(
        "contracts".to_owned(),
        json!([{"guid":"legacy-distraction"}]),
    );
    assert!(
        reader::rows(&catalog, Kind::Contracts)?.is_empty(),
        "the canonical reader excludes an additive legacy root distraction"
    );
    catalog
        .pointer_mut("/projections")
        .and_then(Value::as_object_mut)
        .ok_or("actual projections")?
        .remove("contracts")
        .ok_or("actual contracts table")?;
    assert!(
        reader::rows(&catalog, Kind::Contracts).is_err(),
        "a missing canonical table cannot become an empty successful population"
    );
    Ok(())
}

fn original_owners(forge: &DataForge<'_>) -> TestResult<Vec<(Kind, u32)>> {
    let mut original = Vec::new();
    for (kind, guid, source_type, file) in OWNERS {
        let guid = fixture::guid(guid);
        let mut matches = forge
            .records()
            .filter(|record| record.guid().to_string() == guid);
        let record = matches.next().ok_or("actual physical source owner")?;
        assert!(
            matches.next().is_none(),
            "one physical owner for each controlled GUID"
        );
        assert_eq!(
            record.type_name()?,
            source_type,
            "original mission owner has the expected declaration type"
        );
        assert_eq!(
            record.stored_file_name()?,
            file,
            "original mission owner keeps the exact stored file name"
        );
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "tree belongs to the selected original mission owner"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "original mission owner materializes without source errors"
        );
        original.push((kind, record.index()));
    }
    Ok(original)
}

fn legacy_distraction(catalog: &Value, kind: Kind, original: &Value) -> TestResult {
    let mut catalog = catalog.clone();
    catalog.as_object_mut().ok_or("catalog object")?.insert(
        kind.name().to_owned(),
        json!([{"guid":"not-an-original-guid"}]),
    );
    assert_eq!(
        reader::rows(&catalog, kind)?.as_slice(),
        std::slice::from_ref(original),
        "canonical mission definitions ignore the legacy root entry"
    );
    catalog
        .pointer_mut("/definitions")
        .and_then(Value::as_object_mut)
        .ok_or("canonical definitions")?
        .remove(kind.name())
        .ok_or("actual definition table")?;
    let Err(error) = reader::rows(&catalog, kind) else {
        return Err("the consumer accepted the retired root field".into());
    };
    assert!(
        error.to_string().contains("required canonical table"),
        "the actual missing-table check rejects the legacy distraction"
    );
    Ok(())
}
