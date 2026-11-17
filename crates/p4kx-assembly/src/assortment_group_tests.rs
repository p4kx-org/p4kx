// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::sync::Arc;

use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::loadout_path::{PathRead, XML_LOADOUT_PATH};
use crate::spec::LoadoutSpec;
use crate::spec_loadout::XML_LOADOUT;

use super::Alternative;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn only_a_readable_empty_string_is_an_empty_xml_alternative() -> TestResult {
    for (field, label) in [
        (None, "absent"),
        (Some((TestType::Reference, TestValue::Null)), "null"),
        (
            Some((TestType::Bool, TestValue::Bool(false))),
            "wrong storage",
        ),
        (
            Some((TestType::Locale, TestValue::text(""))),
            "empty localization key",
        ),
        (
            Some((
                TestType::String,
                TestValue::Raw(u32::MAX.to_le_bytes().to_vec()),
            )),
            "unreadable string",
        ),
    ] {
        assert!(
            matches!(read(field)?, Alternative::Unreadable),
            "{label} must keep the assortment issue"
        );
    }
    assert!(
        matches!(
            read(Some((TestType::String, TestValue::text(""))))?,
            Alternative::EmptyXmlPath
        ),
        "an authored empty String selects no layer"
    );
    let path = "Loadouts/Synthetic.xml";
    for storage in [TestType::String, TestType::Locale] {
        assert!(
            matches!(
                read(Some((storage, TestValue::text(path))))?,
                Alternative::Loadout(LoadoutSpec::Xml(actual)) if actual.value() == path
            ),
            "nonempty text follows the existing XML reader"
        );
    }
    Ok(())
}

fn read(field: Option<(TestType, TestValue)>) -> TestResult<Alternative> {
    let mut definition = TestStruct::new(XML_LOADOUT);
    let mut record = TestRecord::new(
        1,
        XML_LOADOUT,
        "SItemPortLoadoutXMLParams.Sample",
        "synthetic/xml.xml",
    );
    if let Some((kind, value)) = field {
        definition = definition.field(XML_LOADOUT_PATH, kind);
        record = record.value(XML_LOADOUT_PATH, value);
    }
    let bytes = DataForgeBuilder::new()
        .structure(definition)
        .record(record)
        .build();
    let forge = DataForge::open(&bytes)?;
    let tree = forge
        .record(0)
        .ok_or("synthetic XML record missing")?
        .tree()?;
    let context = Arc::from(tree.record_path());
    Ok(Alternative::read(
        tree.root(),
        &context,
        &PathRead::new(None, None),
    ))
}
