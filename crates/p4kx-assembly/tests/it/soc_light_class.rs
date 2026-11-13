// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{Entities, SocLightClass, SocLightClassSource};
use p4kx_dataforge::{DataForge, Record, TreeBuilder, Value};
use p4kx_model::SocEntityFile;
use p4kx_testkit::{
    DataForgeBuilder, TestChunkFile, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const ROOT: &str = "EntityClassDefinition";
const COMPONENT: &str = "LightComponentParams";

fn database() -> Vec<u8> {
    let class = |id, name, path| TestRecord::new(id, ROOT, name, path);
    let component = || {
        TestValue::Struct(
            TestInstance::new(COMPONENT).value("useTemperature", TestValue::Bool(false)),
        )
    };
    DataForgeBuilder::new()
        .structure(TestStruct::new("BaseComponent"))
        .structure(
            TestStruct::new(COMPONENT)
                .parent("BaseComponent")
                .field("useTemperature", TestType::Bool),
        )
        .structure(TestStruct::new(ROOT).array(
            "Components",
            TestType::StrongPointer("BaseComponent".to_owned()),
        ))
        .structure(TestStruct::new("OtherType"))
        .record(
            class(1, "EntityClassDefinition.Primary", "source/primary.xml").value(
                "Components",
                TestValue::Array(vec![TestValue::Null, component(), component()]),
            ),
        )
        .record(class(2, "EntityClassDefinition.Twin", "source/a.xml"))
        .record(class(2, "EntityClassDefinition.Twin", "source/b.xml"))
        .record(TestRecord::new(
            3,
            "OtherType",
            "OtherType.Other",
            "source/other.xml",
        ))
        .build()
}

const ENTITIES: &[u8] = br#"<Entities>
<Entity EntityClass="entityclassdefinition.PRIMARY"/>
<Entity EntityClassGUID="00000000-0000-0000-0000-000000000001" EntityClass="Unknown"/>
<Entity EntityClassGUID="00000000-0000-0000-0000-000000000002"/>
<Entity EntityClass="Twin"/>
<Entity EntityClassGUID="00000000-0000-0000-0000-000000000003"/>
<Entity EntityClassGUID="00000000-0000-0000-0000-000000000004" EntityClass="Primary"/>
<Entity EntityClassGUID="bad" EntityClass="Primary"/>
<Entity/>
</Entities>"#;

#[test]
fn class_selection_keeps_exact_guid_name_ambiguity_and_original_component_slots() -> TestResult {
    let original = database();
    let forge = DataForge::open(&original)?;
    let classes = Entities::new(&forge);
    let bytes = TestChunkFile::crch().chunk(0x0004, 3, ENTITIES).bytes();
    let soc = SocEntityFile::parse(&bytes)?;
    let chunk = soc
        .entity_chunks()
        .next()
        .ok_or("original Entity chunk")??;
    let mut rows = chunk
        .entities()
        .map(|entity| SocLightClass::resolve(entity, &classes));
    for _ in 0..2 {
        let Some(SocLightClass::Matched(record)) = rows.next() else {
            return Err("exact class did not match".into());
        };
        assert_eq!(record.index(), 0, "the original physical class");
        let source = SocLightClassSource::read(&record, &mut TreeBuilder::new())?;
        let slots: Vec<_> = source
            .components()
            .map(|(ordinal, component)| (ordinal, component.field("useTemperature")))
            .collect();
        assert_eq!(
            slots,
            [
                (1, Some(&Value::Bool(false))),
                (2, Some(&Value::Bool(false)))
            ],
            "both component occurrences and their authored false values"
        );
    }
    for _ in 0..2 {
        let Some(SocLightClass::Ambiguous(records)) = rows.next() else {
            return Err("duplicate class must stay ambiguous".into());
        };
        assert!(
            records.iter().map(Record::index).eq([1, 2]),
            "original duplicate positions"
        );
    }
    assert!(
        matches!(rows.next(), Some(SocLightClass::UnexpectedRecordType(_))),
        "wrong type"
    );
    assert!(
        matches!(rows.next(), Some(SocLightClass::Unmatched)),
        "nonzero unmatched GUID is explicit"
    );
    assert!(
        matches!(rows.next(), Some(SocLightClass::Invalid)),
        "malformed GUID is explicit"
    );
    assert!(
        matches!(rows.next(), Some(SocLightClass::Absent)),
        "no selector"
    );
    Ok(())
}
