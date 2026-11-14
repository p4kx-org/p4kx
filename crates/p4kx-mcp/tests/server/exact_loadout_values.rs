// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestValue};

use crate::exact_loadout_fixture::Texts;

pub(crate) fn container(texts: &Texts) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SItemPortContainerComponentParams")
            .value("Ports", TestValue::Array(vec![port(texts)])),
    )
}

fn port(texts: &Texts) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SItemPortDef")
            .value("Name", TestValue::text(&texts.port))
            .value("DisplayName", TestValue::text(&texts.display))
            .value("Flags", TestValue::text(&texts.flags))
            .value("MinSize", TestValue::UInt32(2))
            .value("MaxSize", TestValue::UInt32(3))
            .value("Types", TestValue::Array(types(texts)))
            .value("AttachmentImplementation", attachment(&texts.helper)),
    )
}

fn types(texts: &Texts) -> Vec<TestValue> {
    texts
        .types
        .iter()
        .map(|(name, subtypes)| {
            TestValue::Struct(
                TestInstance::new("SItemPortDefTypes")
                    .value("Type", TestValue::text(name))
                    .value(
                        "SubTypes",
                        TestValue::Array(subtypes.iter().map(TestValue::text).collect()),
                    ),
            )
        })
        .collect()
}

fn attachment(name: &str) -> TestValue {
    let helper = TestInstance::new("SItemPortDefHelper").value("Name", TestValue::text(name));
    let node =
        TestInstance::new("SItemPortDefHelperNode").value("Helper", TestValue::Struct(helper));
    TestValue::Struct(
        TestInstance::new("SItemPortDefAttachmentImplementationBone")
            .value("Helper", TestValue::Struct(node)),
    )
}
