// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry, TestLoadout, TestLoadoutEntry};

use crate::exact_loadout_values;
use crate::support::{Install, TestResult};

pub(crate) const ROOT: u128 = 0x0a01;
pub(crate) const CHILD: u128 = 0x0a02;
pub(crate) const MISSING_PORT: &str = "unresolved";

pub(crate) struct Texts {
    pub(crate) class: String,
    pub(crate) child: String,
    pub(crate) file: String,
    pub(crate) port: String,
    pub(crate) display: String,
    pub(crate) flags: String,
    pub(crate) helper: String,
    pub(crate) types: Vec<(String, Vec<String>)>,
    pub(crate) missing: String,
}

impl Texts {
    pub(crate) fn long() -> Self {
        Self {
            class: format!("Root_{}_A", "é".repeat(300)),
            child: format!("Child_{}_tail", "界".repeat(280)),
            file: format!("entities/{}.xml", "f".repeat(330)),
            port: format!("port_{}_end", "p".repeat(1100)),
            display: format!("@Format_{}_end", "é".repeat(300)),
            flags: format!("  {}\\n%%  ", "flag".repeat(100)),
            helper: format!("Helper_{}_end", "é".repeat(300)),
            types: vec![(
                format!("Type_{}_end", "é".repeat(300)),
                vec![format!("Sub_{}_end", "é".repeat(300))],
            )],
            missing: format!("Missing_{}_end", "é".repeat(300)),
        }
    }

    pub(crate) fn short() -> Self {
        Self {
            class: "Exact_Root".to_owned(),
            child: "Exact_Child".to_owned(),
            file: "entities/exact_root.xml".to_owned(),
            port: "attached".to_owned(),
            display: "@Format_Short".to_owned(),
            flags: "visible".to_owned(),
            helper: "Helper".to_owned(),
            types: vec![("Weapon".to_owned(), vec!["Gun".to_owned()])],
            missing: "Exact_Missing".to_owned(),
        }
    }
}

pub(crate) fn install(texts: &Texts) -> TestResult<Install> {
    let root = TestEntity::new(ROOT, &texts.class, &texts.file)
        .component(exact_loadout_values::container(texts))
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::reference(&texts.port, CHILD),
            TestLoadoutEntry::class(MISSING_PORT, &texts.missing),
        ]));
    let child = TestEntity::new(CHILD, &texts.child, "entities/exact_child.xml");
    let decoy = TestEntity::new(
        0x0a03,
        format!("{}_decoy", texts.class),
        "entities/decoy.xml",
    );
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(child.record())
        .record(decoy.record())
        .build();
    Install::new(vec![TestEntry::zstd("Data\\Game2.dcb", database)])
}
