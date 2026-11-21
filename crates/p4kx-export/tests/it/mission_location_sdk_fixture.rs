// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const CLUSTER: u128 = 0x1166_0042;

pub(super) fn entity_cluster() -> DataForgeBuilder {
    let location = "MissionLocationData";
    let owner = "MissionLocationTemplate";
    let cluster = "FixtureEntityCluster";
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(cluster))
        .structure(
            TestStruct::new(location)
                .field("entityClusterMember", TestType::ReferenceTo(cluster.into())),
        )
        .structure(TestStruct::new(owner).field("locationData", TestType::Class(location.into())))
        .record(
            TestRecord::new(
                0x1166_0041,
                owner,
                "MissionLocationTemplate.FixtureCluster",
                "libs/foundry/records/missiondata/pu_locations/fixture/cluster.xml",
            )
            .value(
                "locationData",
                TestValue::Struct(
                    TestInstance::new(location)
                        .value("entityClusterMember", TestValue::Reference(CLUSTER)),
                ),
            ),
        )
        .record(TestRecord::new(
            CLUSTER,
            cluster,
            "FixtureEntityCluster.Target",
            "libs/foundry/records/fixture/cluster.xml",
        ))
}
