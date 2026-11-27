// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::Path;

use p4kx_export::Language;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestRecord, TestStruct,
    TestType, TestValue,
};

pub(crate) fn write(path: &Path, storage: TestType) -> io::Result<()> {
    let mut database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("MissionType").field("LocalisedTypeName", storage));
    for (ordinal, key) in [
        (1, "@fixture_type"),
        (2, "@LOC_PLACEHOLDER"),
        (3, "literal"),
        (4, ""),
    ] {
        database = database.record(
            TestRecord::new(
                ordinal,
                "MissionType",
                format!("MissionType.missiontype.fixture{ordinal}"),
                format!("libs/foundry/records/missiontype/fixture{ordinal}.xml"),
            )
            .value("LocalisedTypeName", TestValue::text(key)),
        );
    }
    let locale = |language: Language, value| {
        TestEntry::stored(
            language.global_ini_path(),
            GlobalIniBuilder::new().text("fixture_type", value).build(),
        )
    };
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database.build()).encrypted())
        .entry(locale(Language::English, "A\\nMission"))
        .entry(locale(Language::French, "Une mission"))
        .write_to(path)
}
