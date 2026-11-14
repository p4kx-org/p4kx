// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestGroup, TestLoadout, TestRecord, TestRequirement,
    assortment_record_requiring,
};

use crate::support::{Install, TestResult};

pub(crate) const CLASS: &str = "Complete_Notes";
pub(crate) const MANY: usize = 65;
const ROOT: u128 = 0x7100;
const FIRST: u128 = 0x7200;
const TAG: u128 = 0x7300;

pub(crate) fn group(ordinal: usize, huge_tail: bool, count: usize) -> String {
    if huge_tail && ordinal == count - 1 {
        format!("Group_{}", "N".repeat(300 << 10))
    } else {
        format!("Group_{ordinal:02}")
    }
}

pub(crate) fn install(count: usize, huge_tail: bool) -> TestResult<Install> {
    let ids: Vec<u128> = (FIRST..).take(count).collect();
    let entity = TestEntity::new(ROOT, CLASS, "entities/complete_notes.xml")
        .loadout(TestLoadout::RandomGenerated(ids.clone()));
    let mut builder = p4kx_testkit::entity_schema(DataForgeBuilder::new()).record(entity.record());
    for (ordinal, guid) in ids.into_iter().enumerate() {
        let group = TestGroup::new(
            group(ordinal, huge_tail, count),
            vec![TestLoadout::Manual(Vec::new())],
        )
        .requiring(TestRequirement::Tags {
            required: vec![TAG],
            banned: Vec::new(),
        });
        builder = builder.record(assortment_record_requiring(
            guid,
            &format!("Complete_{ordinal:02}"),
            None,
            &[group],
        ));
    }
    let database = builder
        .record(TestRecord::new(
            TAG,
            "Tag",
            "Tag.Complete",
            "tags/complete.xml",
        ))
        .build();
    Install::new(vec![TestEntry::zstd("Data\\Game2.dcb", database)])
}
