// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Kind};
use p4kx_extraction::{DataForge, Value as Raw};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::catalog_v3_consumer as reader;
use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult};
use crate::real_archive_verification;

const KIND: Kind = Kind::GameplayProperties;
const TYPE: &str = "CraftingGameplayPropertyDef";
const GUID: u128 = 0x9c01;
const CATEGORY: u128 = 0x9c02;
const ROWS: &str = "/definitions/gameplayProperties/rows";

struct OriginalOwners {
    properties: Vec<u32>,
    foreign: Vec<ForeignOwner>,
}

struct ForeignOwner {
    guid: u128,
    kind: Kind,
    source_type: &'static str,
    index: u32,
}

fn database() -> DataForgeBuilder {
    let property = |name| {
        TestRecord::new(
            GUID,
            TYPE,
            format!("{TYPE}.{name}"),
            format!("libs/foundry/records/crafting/craftedproperties/{name}.xml"),
        )
        .value("propertyName", TestValue::text("@resource_name"))
    };
    fixture::database()
        .structure(TestStruct::new(TYPE).field("propertyName", TestType::Locale))
        .structure(TestStruct::new("BlueprintCategoryRecord"))
        .record(property("First"))
        .record(TestRecord::new(
            CATEGORY,
            "BlueprintCategoryRecord",
            "BlueprintCategoryRecord.Category",
            "libs/foundry/records/crafting/blueprintcategories/category.xml",
        ))
        .record(property("Second"))
}

fn original_source(database: &DataForgeBuilder) -> TestResult<OriginalOwners> {
    let bytes = database.build();
    let forge = DataForge::open(&bytes)?;
    let mut owners = Vec::new();
    for record in forge.records() {
        if record.type_name()? == TYPE {
            assert_eq!(
                record.guid().to_string(),
                fixture::guid(GUID),
                "actual equal nominal property GUIDs"
            );
            let tree = record.tree()?;
            assert_eq!(
                tree.root().field("propertyName"),
                Some(&Raw::Locale("@resource_name")),
                "actual authored Locale"
            );
            owners.push(record.index());
        }
    }
    assert_eq!(owners.len(), 2, "two actual physical property owners");
    assert!(
        owners.is_sorted_by(|left, right| left < right),
        "source positions are distinct and ordered"
    );
    Ok(OriginalOwners {
        properties: owners,
        foreign: original_foreign(&forge)?,
    })
}

fn original_foreign(forge: &DataForge<'_>) -> TestResult<Vec<ForeignOwner>> {
    let mut foreign = Vec::new();
    for (guid, kind, source_type) in [
        (fixture::RESOURCE, Kind::ResourceTypes, "ResourceType"),
        (
            CATEGORY,
            Kind::BlueprintCategories,
            "BlueprintCategoryRecord",
        ),
        (
            fixture::QUANTIZATION,
            Kind::QualityQuantizations,
            "CraftingQualityQuantizationRecord",
        ),
    ] {
        let expected = fixture::guid(guid);
        let mut matching = forge
            .records()
            .filter(|record| record.guid().to_string() == expected);
        let record = matching.next().ok_or("actual foreign collision owner")?;
        assert!(
            matching.next().is_none(),
            "the fixture supplies one original collision owner"
        );
        assert_eq!(
            record.type_name()?,
            source_type,
            "the foreign source family is genuine"
        );
        foreign.push(ForeignOwner {
            guid,
            kind,
            source_type,
            index: record.index(),
        });
    }
    Ok(foreign)
}

#[test]
fn canonical_property_physical_ties_keep_foreign_family_identity_guards() -> TestResult {
    let database = database();
    let original = original_source(&database)?;
    let (_directory, archive) = fixture::open(&database, DcbOrigin::Plain)?;
    let mut options = fixture::options();
    options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    let built = p4kx_export::build_catalog(&archive, &options)?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    assert_eq!(
        reader::rows(&catalog, KIND)?.len(),
        2,
        "both physical alternatives are actually exported"
    );
    for (row, index) in reader::rows(&catalog, KIND)?
        .iter()
        .zip(&original.properties)
    {
        assert_eq!(
            reader::field(row, KIND, "recordIndex")?,
            &json!(index),
            "each exported tie keeps its actual original Record index"
        );
    }
    super::check(&catalog, KIND)?;
    real_archive_verification::check_unique_guids(&catalog)?;
    assert_foreign_aliases(&catalog, &original.foreign)?;
    assert_repeated_owner(&catalog)?;
    assert_foreign_type(&catalog)
}

fn assert_repeated_owner(catalog: &Value) -> TestResult {
    let mut repeated = catalog.clone();
    let row = reader::rows(catalog, KIND)?
        .last()
        .ok_or("actual property row")?
        .clone();
    repeated
        .pointer_mut(ROWS)
        .and_then(Value::as_array_mut)
        .ok_or("actual property rows")?
        .push(row);
    let order = reader::rows(&repeated, KIND)?
        .iter()
        .map(|row| {
            Ok((
                reader::identity(row, KIND)?,
                reader::field(row, KIND, "recordIndex")?
                    .as_u64()
                    .ok_or("physical Record index")?,
            ))
        })
        .collect::<TestResult<Vec<_>>>()?;
    assert!(
        order.is_sorted(),
        "the duplicate mutation preserves nominal and physical order"
    );
    let error = real_archive_verification::check_unique_guids(&repeated)
        .err()
        .ok_or("the duplicate physical owner must be rejected")?;
    assert_eq!(
        error.to_string(),
        "one physical definition cannot occur twice",
        "the physical duplicate guard rejects the otherwise ordered rows"
    );
    Ok(())
}

fn assert_foreign_type(catalog: &Value) -> TestResult {
    let mut wrong = catalog.clone();
    *wrong
        .pointer_mut(&format!("{ROWS}/0/sourceType/value"))
        .ok_or("actual source type")? = json!("ResourceType");
    assert!(
        real_archive_verification::check_unique_guids(&wrong).is_err(),
        "the allowance requires the actual property source family"
    );
    Ok(())
}

fn assert_foreign_aliases(catalog: &Value, original: &[ForeignOwner]) -> TestResult {
    for owner in original {
        let expected = fixture::guid(owner.guid);
        let mut present = 0;
        for row in reader::rows(catalog, owner.kind)? {
            if reader::identity(row, owner.kind)? == expected {
                assert_eq!(
                    reader::field(row, owner.kind, "recordIndex")?,
                    &json!(owner.index),
                    "the foreign canonical row keeps its original physical owner"
                );
                assert_eq!(
                    reader::field(row, owner.kind, "sourceType")?.as_str(),
                    Some(owner.source_type),
                    "the collision target has the actual original family"
                );
                present += 1;
            }
        }
        assert_eq!(
            present, 1,
            "the actual foreign {} definition exists once before mutation",
            owner.kind
        );
        let mut alias = catalog.clone();
        for row in alias
            .pointer_mut(ROWS)
            .and_then(Value::as_array_mut)
            .ok_or("actual physical rows")?
        {
            *row.pointer_mut("/guid/value")
                .ok_or("actual property GUID")? = json!(expected);
        }
        super::check(&alias, KIND)?;
        assert!(
            real_archive_verification::check_unique_guids(&alias).is_err(),
            "the C2 physical allowance does not waive an actual {} GUID collision",
            owner.kind
        );
    }
    Ok(())
}
