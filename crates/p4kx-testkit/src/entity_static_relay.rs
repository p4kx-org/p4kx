// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_schema::entity_schema;
use crate::{DataForgeBuilder, TestEntity, TestEntry, TestModel, TestNode, TestPort};

/// Returns a relay and fuse whose static port uses two translated, rotated helper nodes.
#[must_use]
pub fn static_relay_entries() -> Vec<TestEntry> {
    let fuse_guid = 2;
    let relay = TestEntity::new(1, "TEST_Relay", "entities/test_relay.xml")
        .geometry("objects/test/relay.cgf", None)
        .port(
            TestPort::new("slot")
                .entity_attachment(true)
                .default_item(fuse_guid),
        );
    let fuse = TestEntity::new(fuse_guid, "TEST_Fuse", "entities/test_fuse.xml")
        .geometry("objects/test/fuse.cgf", None);
    let forge = entity_schema(DataForgeBuilder::new())
        .record(relay.record())
        .record(fuse.record())
        .build();
    let relay_model = relay_with_rotated_socket();
    let fuse_model = TestModel::cube();
    vec![
        TestEntry::stored("Data/Game2.dcb", forge),
        TestEntry::stored("Data/objects/test/relay.cgf", relay_model.cgf()),
        TestEntry::stored("Data/objects/test/fuse.cgf", fuse_model.cgf()),
        TestEntry::stored("Data/objects/test/fuse.cgfm", fuse_model.cgfm()),
    ]
}

fn relay_with_rotated_socket() -> TestModel {
    TestModel::cube()
        .with_nodes(vec![
            TestNode {
                name: "socket frame".into(),
                parent: None,
                local: [
                    [1.0, 0.0, 0.0, 7.0],
                    [0.0, 0.0, -1.0, 11.0],
                    [0.0, 1.0, 0.0, 13.0],
                ],
                kind: 2,
            },
            TestNode {
                name: "slot".into(),
                parent: Some(0),
                local: [
                    [0.0, -1.0, 0.0, 2.0],
                    [1.0, 0.0, 0.0, 3.0],
                    [0.0, 0.0, 1.0, 4.0],
                ],
                kind: 2,
            },
        ])
        .helpers_only()
}
