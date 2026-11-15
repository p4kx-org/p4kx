// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_extraction::Soundbank;
use p4kx_testkit::{TestBank, TestContainerKind, TestHircObject, TestNodeBase, wwise_short_id};
use serde_json::Value;

use super::{
    CONTAINER, EMBEDDED, EVENT, FIRST_SOUND, FIRST_WEM, ID_BYTES, MISSING, PLAY, SECOND_SOUND,
    SECOND_WEM, Site, TXTP_FILE, TestResult, check_receipt, export, fixture, pcm, referenced_sites,
    rendered_commands, selection_field,
};

const SEQUENCE: u32 = 11;
const INNER_CHOICE: u32 = 12;
const SELECTED_BLEND: u32 = 13;
const UNRELATED_BLEND: u32 = 14;
const UNRELATED_SOUND: u32 = 22;
const UNRELATED_WEM: u32 = 102;
const PLAIN_NODE_LEN: usize = 31;
const MODE_SIZE: usize = size_of::<u8>();
const MODE_OFFSET: usize = PLAIN_NODE_LEN + 4 * size_of::<u16>() + 3 * size_of::<f32>() + 2;
const RANDOM_SEQUENCE: &str = "bnkHircRandomSequence";
const CONTAINERS: [ContainerCase<'static>; 4] = [
    ContainerCase {
        id: CONTAINER,
        kind: TestContainerKind::Random,
        children: &[MISSING, SEQUENCE],
        followed: &[1],
    },
    ContainerCase {
        id: SEQUENCE,
        kind: TestContainerKind::Sequence,
        children: &[INNER_CHOICE, UNRELATED_BLEND],
        followed: &[0],
    },
    ContainerCase {
        id: INNER_CHOICE,
        kind: TestContainerKind::Random,
        children: &[SELECTED_BLEND, SECOND_SOUND],
        followed: &[0, 1],
    },
    ContainerCase {
        id: SELECTED_BLEND,
        kind: TestContainerKind::Blend,
        children: &[FIRST_SOUND, FIRST_SOUND],
        followed: &[0, 1],
    },
];

#[test]
fn selection_attests_to_followed_ancestor_and_descendant_container_children() -> TestResult {
    let bank = nested_bank();
    let fixture = fixture(&bank, false)?;
    let report = export(&fixture, &[])?;
    let file = report
        .pointer(TXTP_FILE)
        .ok_or("written nested TXTP file")?;
    let field = selection_field(file)?;
    let origin = file["fieldOrigins"].get(&field).ok_or("selection origin")?;
    let text = check_receipt(&fixture.out, file, origin)?;
    check_commands(&text);
    assert_eq!(file["breaks"], 1, "the outer missing child leaves a break");
    assert_eq!(
        origin["containsEncryptedSource"],
        Value::Null,
        "the contributing plain bank supplies partial evidence"
    );
    assert_eq!(
        origin["sourceSetComplete"], false,
        "the complete source set is still unresolved"
    );
    assert_eq!(
        referenced_sites(file, origin)?,
        original_sites(&bank, &CONTAINERS)?,
        "only the exact physical child words of the inner selection's commands are cited"
    );
    Ok(())
}

fn check_commands(text: &str) {
    let commands: Vec<_> = rendered_commands(text).collect();
    assert_eq!(
        commands,
        [
            "Play_Topology_100.wav",
            "Play_Topology_100.wav",
            "group = -L2 #@layer-v",
            "Play_Topology_101.wav",
            "group = -R2>1",
            "Play_Topology_102.wav",
            "Play_Topology_102.wav",
            "group = -L2 #@layer-v",
            "group = -S2",
        ],
        "the outer choice collapses and the separate sequence branch still renders"
    );
}

pub(super) struct ContainerCase<'a> {
    pub(super) id: u32,
    pub(super) kind: TestContainerKind,
    pub(super) children: &'a [u32],
    pub(super) followed: &'a [usize],
}

pub(super) fn original_sites(
    bytes: &[u8],
    containers: &[ContainerCase<'_>],
) -> Result<BTreeSet<Site<'static>>, Box<dyn Error>> {
    let bank = Soundbank::parse(bytes)?;
    let version = bank.version_byte_range().ok_or("bank version word")?;
    let mut sites = BTreeSet::from([Site::word("bnkBkhd", "version", version)]);
    for container in containers {
        let object = bank.object(container.id).ok_or("original container")?;
        container.insert_sites(
            bytes,
            object.byte_range().start,
            object.id_byte_range().end,
            &mut sites,
        )?;
    }
    Ok(sites)
}

impl ContainerCase<'_> {
    fn insert_sites(
        &self,
        bytes: &[u8],
        type_offset: usize,
        body: usize,
        sites: &mut BTreeSet<Site<'static>>,
    ) -> TestResult {
        let layout = ContainerLayout::of(self.kind);
        assert_eq!(
            bytes.get(type_offset),
            Some(&layout.type_code),
            "HIRC type for owner {}",
            self.id
        );
        sites.insert(Site::word(
            "bnkHircObject",
            "typeCode",
            type_offset..type_offset + 1,
        ));
        if let Some(value) = layout.mode {
            let start = body + MODE_OFFSET;
            assert_eq!(bytes.get(start), Some(&value), "mode for owner {}", self.id);
            sites.insert(Site::word(
                layout.namespace,
                "randomSequenceMode",
                start..start + MODE_SIZE,
            ));
        }
        let count = body + layout.count_offset;
        let first = count + ID_BYTES;
        assert_eq!(
            bytes.get(count..first),
            Some(u32::try_from(self.children.len())?.to_le_bytes().as_slice()),
            "declared child count for owner {}",
            self.id
        );
        sites.insert(Site::word(
            layout.namespace,
            "containerChildCount",
            count..first,
        ));
        for (ordinal, id) in self.children.iter().enumerate() {
            let start = first + ordinal * ID_BYTES;
            let end = start + ID_BYTES;
            assert_eq!(
                bytes.get(start..end),
                Some(id.to_le_bytes().as_slice()),
                "child word {ordinal} for owner {}",
                self.id
            );
            if self.followed.contains(&ordinal) {
                sites.insert(Site {
                    ordinal: Some(ordinal),
                    ..Site::word(layout.namespace, "containerChildId", start..end)
                });
            }
        }
        Ok(())
    }
}

struct ContainerLayout {
    namespace: &'static str,
    type_code: u8,
    count_offset: usize,
    mode: Option<u8>,
}

impl ContainerLayout {
    const fn of(kind: TestContainerKind) -> Self {
        match kind {
            TestContainerKind::Random | TestContainerKind::Sequence => Self {
                namespace: RANDOM_SEQUENCE,
                type_code: 5,
                count_offset: MODE_OFFSET + 2 * MODE_SIZE,
                mode: Some(if matches!(kind, TestContainerKind::Sequence) {
                    1
                } else {
                    0
                }),
            },
            TestContainerKind::Switch => Self {
                namespace: "bnkHircSwitchContainer",
                type_code: 6,
                count_offset: PLAIN_NODE_LEN + 2 * MODE_SIZE + 2 * ID_BYTES,
                mode: None,
            },
            TestContainerKind::Blend => Self {
                namespace: "bnkHircBlendContainer",
                type_code: 9,
                count_offset: PLAIN_NODE_LEN,
                mode: None,
            },
        }
    }
}

fn nested_bank() -> Vec<u8> {
    let mut bank = TestBank::new(wwise_short_id("Topology"))
        .version(154)
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1]))
        .object(TestHircObject::action(1, PLAY, CONTAINER));
    for container in &CONTAINERS {
        bank = bank.object(TestHircObject::container(
            container.kind,
            container.id,
            container.children,
            TestNodeBase::Plain,
        ));
    }
    bank = bank.object(TestHircObject::container(
        TestContainerKind::Blend,
        UNRELATED_BLEND,
        &[UNRELATED_SOUND, UNRELATED_SOUND],
        TestNodeBase::Plain,
    ));
    for (id, wem) in [
        (FIRST_SOUND, FIRST_WEM),
        (SECOND_SOUND, SECOND_WEM),
        (UNRELATED_SOUND, UNRELATED_WEM),
    ] {
        bank = bank
            .wem(wem, pcm())
            .object(TestHircObject::sound(id, wem, EMBEDDED, 0));
    }
    bank.bytes()
}
