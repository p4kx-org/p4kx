// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestAtl, TestBank, TestCodec, TestEntity, TestEntry, TestHircObject,
    TestInstance, TestStruct, TestType, TestValue, TestWem,
};

use crate::support::{Install, TestResult};

pub(crate) const COMPONENT: &str = "ExactAudioComponent";
pub(crate) const WEM: u32 = 7819;

#[derive(Clone, Copy)]
pub(crate) enum Encoding {
    Text,
    CryXml,
}

pub(crate) struct Texts {
    pub(crate) trigger: String,
    pub(crate) later: String,
    pub(crate) events: Vec<String>,
    pub(crate) class: String,
    pub(crate) field: String,
    pub(crate) record_file: String,
    pub(crate) bank_file: String,
}

impl Texts {
    pub(crate) fn long() -> Self {
        Self {
            trigger: format!("Exact_Reusable_{}Z", "é".repeat(120)),
            later: format!("Exact_Long_{}_tail", "é".repeat(150)),
            events: vec![
                format!("Play_Exact_{}_good", "é".repeat(300)),
                format!("Play_Exact_{}_broken", "界".repeat(160)),
            ],
            class: format!("Audio_{}_tail", "é".repeat(300)),
            field: format!("audioTrigger_{}", "field".repeat(80)),
            record_file: format!("entities/{}.xml", "r".repeat(300)),
            bank_file: format!("{}.bnk", "b".repeat(300)),
        }
    }

    pub(crate) fn bank_path(&self) -> String {
        format!("Data\\Sounds\\wwise\\{}", self.bank_file)
    }
}

pub(crate) fn install(texts: &Texts, encoding: Encoding) -> TestResult<Install> {
    let component = TestStruct::new(COMPONENT)
        .parent("DataForgeComponentParams")
        .field(&texts.field, TestType::String);
    let entity =
        TestEntity::new(0x0b01, &texts.class, &texts.record_file).component(TestValue::Struct(
            TestInstance::new(COMPONENT).value(&texts.field, TestValue::text(&texts.trigger)),
        ));
    let database = p4kx_testkit::audio_schema(p4kx_testkit::entity_schema(DataForgeBuilder::new()))
        .structure(component)
        .record(entity.record())
        .build();
    let event_names: Vec<&str> = texts.events.iter().map(String::as_str).collect();
    let first = texts.events.first().ok_or("actual event")?;
    let atl = TestAtl::new("Exact")
        .trigger(&texts.trigger, &event_names)
        .trigger(&texts.later, &[first.as_str()])
        .bank(&texts.bank_file);
    let definition = match encoding {
        Encoding::Text => atl.to_xml().into_bytes(),
        Encoding::CryXml => atl.to_cryxml(),
    };
    Install::new(vec![
        TestEntry::zstd("Data\\Game2.dcb", database),
        TestEntry::stored("Data\\Libs\\GameAudio\\Exact.xml", definition),
        TestEntry::stored(texts.bank_path(), bank(texts)?),
    ])
}

fn bank(texts: &Texts) -> TestResult<Vec<u8>> {
    let mut bank = TestBank::new(p4kx_testkit::wwise_short_id(
        texts
            .bank_file
            .strip_suffix(".bnk")
            .ok_or("fixture bank extension")?,
    ))
    .wem(
        WEM,
        TestWem::pcm(TestCodec::Pcm, 1, 8000, &[0, 1, -1, 7]).bytes(),
    )
    .object(TestHircObject::action(1, 0x0403, 100))
    .object(TestHircObject::sound(100, WEM, 0, 0));
    for (position, event) in texts.events.iter().enumerate() {
        let action = if position == 0 { 1 } else { 777 };
        bank = bank.object(TestHircObject::event(
            p4kx_testkit::wwise_short_id(event),
            &[action],
        ));
    }
    Ok(bank.bytes())
}
