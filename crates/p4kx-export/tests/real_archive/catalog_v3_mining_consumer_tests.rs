// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Kind, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};
use serde_json::{Value, json};

use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

#[test]
fn both_migrated_mining_families_have_nonempty_canonical_rows_and_sourced_identities() -> TestResult
{
    let (_folder, archive) = common::open(&database(), DcbOrigin::Plain)?;
    let mut options = common::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = build_catalog(&archive, &options)?;
    let mut catalog: Value = serde_json::from_slice(built.json())?;
    for (kind, guid) in [
        (Kind::RefiningProcesses, 0xC111),
        (Kind::MiningGlobalParams, 0xC112),
    ] {
        catalog
            .as_object_mut()
            .ok_or("canonical catalog object")?
            .insert(
                kind.name().to_owned(),
                json!([{"guid":"unrelated-legacy-distraction"}]),
            );
        let rows = reader::rows(&catalog, kind)?;
        assert_eq!(rows.len(), 1, "one actual nonempty migrated family row");
        assert_eq!(
            reader::identity(&rows[0], kind)?,
            common::guid(guid),
            "the actual physical source identity is read from its cell"
        );
        assert_eq!(
            reader::field(&rows[0], kind, "className")?,
            &json!("Only"),
            "sourced metadata is read through the actual consumer"
        );
    }
    Ok(())
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("RefiningProcess"))
        .structure(TestStruct::new("MiningGlobalParams"))
        .record(TestRecord::new(
            0xC111,
            "RefiningProcess",
            "RefiningProcess.Only",
            "libs/foundry/records/refiningprocess/only.xml",
        ))
        .record(TestRecord::new(
            0xC112,
            "MiningGlobalParams",
            "MiningGlobalParams.Only",
            "libs/foundry/records/mining/miningglobalparams_only.xml",
        ))
}
