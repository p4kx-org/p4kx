// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeOrigin, Language, SourceEncryption, StructValue, Value, ValueError,
};
use p4kx_testkit::{
    DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};

use super::super::structures;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

const GENERATOR: u128 = 0x7181;
const BROKER: u128 = 0x7182;
const CONTRACT: u128 = 0x7183;
const BAD: u32 = u32::MAX - 1;
pub(super) const WINNER: &str = "@complete_fallback";

#[derive(Clone, Copy)]
pub(super) enum Case {
    Unreadable,
    Absent,
    Fallback,
}

pub(super) fn open(case: Case, input: Input) -> TestResult<Fixture> {
    let data = structures::structures(
        DataForgeBuilder::new()
            .version(8)
            .enumeration("ECurrency", ["UEC"]),
    )
    .record(generator(case))
    .record(broker(case))
    .build();
    assert!(
        u64::from(BAD) > u64::try_from(data.len())?,
        "the bad offset is beyond every original byte"
    );
    let english = GlobalIniBuilder::new().text("complete_fallback", "Readable fallback");
    let fixture = Fixture::open(
        &data,
        input,
        vec![TestEntry::stored(
            Language::English.global_ini_path(),
            english.build(),
        )],
    )?;
    original(&fixture, case, input)?;
    Ok(fixture)
}

fn generator(case: Case) -> TestRecord {
    let mut contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(CONTRACT))
        .value("missionBrokerEntry", TestValue::Reference(BROKER));
    if matches!(case, Case::Fallback) {
        let parameter = TestInstance::new("ContractStringParam")
            .value("param", TestValue::text("Title"))
            .value("value", bad());
        contract = contract.value(
            "paramOverrides",
            TestValue::Struct(TestInstance::new("ContractParamOverrides").value(
                "stringParamOverrides",
                TestValue::Array(vec![TestValue::Struct(parameter)]),
            )),
        );
    }
    let handler = TestInstance::new("ContractGeneratorHandler_List").value(
        "contracts",
        TestValue::Array(vec![TestValue::Struct(contract)]),
    );
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.MissingText",
        "libs/foundry/records/contracts/contractgenerator/missing_text.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn broker(case: Case) -> TestRecord {
    let (title, description) = match case {
        Case::Unreadable => (bad(), bad()),
        Case::Absent => (
            TestValue::text("@LOC_PLACEHOLDER"),
            TestValue::text("@LOC_PLACEHOLDER"),
        ),
        Case::Fallback => (TestValue::text(WINNER), TestValue::text("@LOC_PLACEHOLDER")),
    };
    TestRecord::new(
        BROKER,
        "MissionBrokerEntry",
        "MissionBrokerEntry.MissingText",
        "libs/foundry/records/missionbroker/missing_text.xml",
    )
    .value("title", title)
    .value("description", description)
}

fn bad() -> TestValue {
    TestValue::Raw(BAD.to_le_bytes().to_vec())
}

fn original(fixture: &Fixture, case: Case, input: Input) -> TestResult {
    original_input(fixture, input)?;
    let forge = fixture.entry.open()?;
    let generator = forge.record(0).ok_or("original Generator")?;
    let broker = forge.record(1).ok_or("original Broker")?;
    assert_eq!(
        generator.guid().to_string(),
        health_origin_fixture::guid(GENERATOR),
        "Record zero is the original Generator owner"
    );
    assert_eq!(
        broker.guid().to_string(),
        health_origin_fixture::guid(BROKER),
        "Record one is the original Broker dependency"
    );
    let generator_tree = generator.tree()?;
    let broker_tree = broker.tree()?;
    assert!(
        generator_tree.is_from(&generator) && broker_tree.is_from(&broker),
        "both original trees belong to their actual admitted Records"
    );
    let handler = first(generator_tree.root(), "generators")?;
    let contract = first(handler, "contracts")?;
    assert!(
        matches!(contract.field("id"), Some(Value::Guid(guid))
        if guid.to_string() == health_origin_fixture::guid(CONTRACT)),
        "the selected contract stores its own authored identifier"
    );
    assert!(
        matches!(contract.field("missionBrokerEntry"), Some(Value::Reference(reference))
        if reference.record_index() == broker.index() && reference.location().is_none()),
        "the authored Reference selects this exact original Broker Record"
    );
    for (ordinal, name) in [(0, "title"), (1, "description")] {
        let field = broker_tree
            .fields()
            .get(ordinal)
            .ok_or("actual original key slot")?;
        assert_eq!(
            field.name(),
            name,
            "the source schema fixes the actual root ordinal"
        );
        assert!(
            broker_tree
                .field(name)
                .is_some_and(|value| std::ptr::eq(field.value(), value)),
            "the observed key is the exact original field occurrence"
        );
    }
    original_keys(case, contract, &generator_tree, &broker_tree)
}

fn original_input(fixture: &Fixture, input: Input) -> TestResult {
    match fixture.entry.origin() {
        DataForgeOrigin::Archive(origin) => assert_eq!(
            origin.encryption(),
            if matches!(input, Input::Encrypted) {
                SourceEncryption::Encrypted
            } else {
                SourceEncryption::Unencrypted
            },
            "the actual input encryption is independent of the result"
        ),
        DataForgeOrigin::File(_) => assert!(
            matches!(input, Input::Standalone),
            "only the standalone fixture has an external file origin"
        ),
        _ => return Err("an actual Archive or standalone fixture".into()),
    }
    Ok(())
}

fn original_keys(
    case: Case,
    contract: &StructValue<'_>,
    generator_tree: &p4kx_extraction::RecordTree<'_>,
    broker_tree: &p4kx_extraction::RecordTree<'_>,
) -> TestResult {
    match case {
        Case::Unreadable => {
            assert_eq!(
                generator_tree.error_count(),
                0,
                "all Generator source fields are readable"
            );
            assert_eq!(
                broker_tree.error_count(),
                2,
                "only the two encoded Broker key offsets fail decoding"
            );
            error(broker_tree.field("title"));
            error(broker_tree.field("description"));
        }
        Case::Absent => {
            assert_eq!(
                generator_tree.error_count(),
                0,
                "all Generator source fields are readable"
            );
            assert_eq!(
                broker_tree.error_count(),
                0,
                "the Broker source has no decoder failures"
            );
            for name in ["title", "description"] {
                assert_eq!(
                    broker_tree.field(name),
                    Some(&Value::Locale("@LOC_PLACEHOLDER")),
                    "the absent key comes from the actual readable placeholder"
                );
            }
        }
        Case::Fallback => fallback_original(contract, generator_tree, broker_tree)?,
    }
    Ok(())
}

fn fallback_original(
    contract: &StructValue<'_>,
    generator: &p4kx_extraction::RecordTree<'_>,
    broker: &p4kx_extraction::RecordTree<'_>,
) -> TestResult {
    assert_eq!(
        generator.error_count(),
        1,
        "only the earlier Generator key offset fails decoding"
    );
    assert_eq!(
        broker.error_count(),
        0,
        "the selected Broker fallback is fully readable"
    );
    let Some(Value::Struct(parameters)) = contract.field("paramOverrides") else {
        return Err("actual parameters".into());
    };
    let parameter = first(parameters, "stringParamOverrides")?;
    assert_eq!(
        parameter.field("param"),
        Some(&Value::String("Title")),
        "the unreadable original value actually matches the Title selector"
    );
    error(parameter.field("value"));
    assert_eq!(
        broker.field("title"),
        Some(&Value::Locale(WINNER)),
        "the successful fallback key is authored in the original Broker field"
    );
    Ok(())
}

fn error(value: Option<&Value<'_>>) {
    assert!(
        matches!(value, Some(Value::Error(ValueError::InvalidString { offset, .. }))
        if *offset == BAD),
        "the original selected key has the exact encoded decoder failure"
    );
}

fn first<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Value::Array(values)) = owner.field(name) else {
        return Err("actual array".into());
    };
    let [Value::Struct(value)] = values.as_ref() else {
        return Err("one actual owned Struct".into());
    };
    Ok(value)
}
