// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;

use p4kx_extraction::Soundbank;
use p4kx_testkit::{
    ArchiveBuilder, TestAtl, TestBank, TestContainerKind, TestEntry, TestHircObject, TestNodeBase,
    wwise_short_id,
};
use serde_json::Value;

use super::{
    BANK_PATH, CONTAINER, EMBEDDED, EVENT, FIRST_SOUND, FIRST_WEM, Fixture, ID_BYTES, PLAY,
    SECOND_SOUND, SECOND_WEM, Site, TXTP_FILE, TestResult, check_receipt, export, pcm,
    selection_origin, selection_uses,
};

const ROOT_NAME: &str = "SelectionRoute";
const ROOT_PATH: &str = "Data/Sounds/wwise/SelectionRoute.bnk";
const RELAY_NAME: &str = "SelectionRelay";
const RELAY_PATH: &str = "Data/Sounds/wwise/SelectionRelay.bnk";
const CHOICE_BANK: &str = "Topology";
const POST_EVENT: u16 = 0x2103;
const POST_ACTION: u32 = 1;
const SIBLING_ACTION: u32 = 2;
const PLAY_ACTION: u32 = 3;
const UNRESOLVED_ACTION: u32 = 4;
const POSTED_EVENT: u32 = 500;
const SIBLING_SOUND: u32 = 22;
const SIBLING_WEM: u32 = 102;
const ROOT_ACTIONS: [u32; 4] = [UNRESOLVED_ACTION, POST_ACTION, SIBLING_ACTION, POST_ACTION];
const POSTED_ACTIONS: [u32; 1] = [PLAY_ACTION];
const FIRST_RENDERED_ACTION: usize = 1;
const EVENT_NAMESPACE: &str = "bnkHircEvent";
const ACTION_NAMESPACE: &str = "bnkHircAction";
const PLAY_PARAMETERS_BYTES: usize = 4;

#[test]
fn selection_attests_to_the_event_and_action_occurrences_that_reach_it() -> TestResult {
    let root = root_bank();
    let relay = relay_bank();
    let choice = choice_bank();
    let fixture = fixture(&root, &relay, &choice)?;
    let report = export(&fixture, &[])?;
    let file = report.pointer(TXTP_FILE).ok_or("written TXTP file")?;
    let origin = selection_origin(file)?;
    let text = check_receipt(&fixture.out, file, origin)?;

    assert_eq!(file["breaks"], 1, "the absent action leaves a broken link");
    assert_eq!(
        text.lines()
            .filter(|line| line.trim() == "group = -R2>1")
            .count(),
        2,
        "repeated PlayEvent actions render independent choices"
    );
    let sibling = format!("{EVENT}_{SIBLING_WEM}.wav");
    assert!(
        text.lines().any(|line| line.trim() == sibling),
        "the successful sibling route is also rendered"
    );
    assert_eq!(
        origin["containsEncryptedSource"], true,
        "the encrypted upstream route contributes to the plain selection bank"
    );
    assert_eq!(
        origin["sourceSetComplete"], false,
        "route words do not prove every source dependency"
    );
    assert_eq!(
        referenced_route(file, origin)?,
        original_route(&root, &relay, &choice)?,
        "the first selection cites only the exact Event and Action words on its route"
    );
    Ok(())
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RouteSite<'a> {
    path: &'a str,
    site: Site<'a>,
}

fn referenced_route<'a>(
    file: &'a Value,
    origin: &'a Value,
) -> Result<BTreeSet<RouteSite<'a>>, Box<dyn Error>> {
    let mut sites = BTreeSet::new();
    for observation in selection_uses(file, origin)? {
        let observation = observation?;
        if !observation.is_route() && observation.namespace() != Some("bnkBkhd") {
            continue;
        }
        let path = observation.path()?;
        let encryption = if path == ROOT_PATH {
            "encrypted"
        } else {
            "unencrypted"
        };
        assert_eq!(
            observation.physical["storedEncryption"], encryption,
            "physical route encryption for {path}"
        );
        assert!(
            sites.insert(RouteSite {
                path,
                site: Site::from_use(observation.site)?,
            }),
            "one selection reference per physical route word"
        );
    }
    Ok(sites)
}

fn original_route(
    root: &[u8],
    relay: &[u8],
    choice: &[u8],
) -> Result<BTreeSet<RouteSite<'static>>, Box<dyn Error>> {
    let root = Soundbank::parse(root)?;
    let relay = Soundbank::parse(relay)?;
    let choice = Soundbank::parse(choice)?;
    let mut sites = BTreeSet::new();
    for (path, bank) in [
        (ROOT_PATH, &root),
        (RELAY_PATH, &relay),
        (BANK_PATH, &choice),
    ] {
        sites.insert(RouteSite {
            path,
            site: Site::word(
                "bnkBkhd",
                "version",
                bank.version_byte_range().ok_or("BKHD version")?,
            ),
        });
    }
    sites.extend(original_event(
        &root,
        ROOT_PATH,
        wwise_short_id(EVENT),
        &ROOT_ACTIONS,
        FIRST_RENDERED_ACTION,
    )?);
    sites.extend(original_event(
        &relay,
        RELAY_PATH,
        POSTED_EVENT,
        &POSTED_ACTIONS,
        0,
    )?);
    sites.extend(original_action(
        &root,
        ROOT_PATH,
        POST_ACTION,
        POST_EVENT,
        POSTED_EVENT,
    )?);
    sites.extend(original_relay_play(&relay)?);
    Ok(sites)
}

fn original_relay_play(bank: &Soundbank<&[u8]>) -> Result<[RouteSite<'static>; 3], Box<dyn Error>> {
    let [kind, target] = original_action(bank, RELAY_PATH, PLAY_ACTION, PLAY, CONTAINER)?;
    let bank_id_start = target.site.end + PLAY_PARAMETERS_BYTES;
    let bank_id_end = bank_id_start + ID_BYTES;
    let play_body = bank.object(PLAY_ACTION).ok_or("the relay Play")?;
    let body_start = play_body.id_byte_range().start;
    assert_eq!(
        bank.object_body(play_body)
            .get(bank_id_start - body_start..bank_id_end - body_start),
        Some(wwise_short_id(CHOICE_BANK).to_le_bytes().as_slice()),
        "the Play specifies the different physical bank"
    );
    Ok([
        kind,
        target,
        RouteSite {
            path: RELAY_PATH,
            site: Site::word(ACTION_NAMESPACE, "playBankId", bank_id_start..bank_id_end),
        },
    ])
}

fn original_event(
    bank: &Soundbank<&[u8]>,
    path: &'static str,
    id: u32,
    actions: &[u32],
    ordinal: usize,
) -> Result<[RouteSite<'static>; 2], Box<dyn Error>> {
    let event = bank.object(id).ok_or("original Event")?;
    let body = bank.object_body(event);
    let count = event.id_byte_range().end;
    let action_list = count + size_of::<u8>();
    assert_eq!(
        body.get(ID_BYTES),
        Some(&u8::try_from(actions.len())?),
        "the authored action count"
    );
    let start = action_list + ordinal * ID_BYTES;
    let end = start + ID_BYTES;
    let body_start = event.id_byte_range().start;
    let action = actions.get(ordinal).ok_or("fixture action ordinal")?;
    assert_eq!(
        body.get(start - body_start..end - body_start),
        Some(action.to_le_bytes().as_slice()),
        "the original action word at ordinal {ordinal}"
    );
    Ok([
        RouteSite {
            path,
            site: Site::word(EVENT_NAMESPACE, "actionCount", count..action_list),
        },
        RouteSite {
            path,
            site: Site {
                ordinal: Some(ordinal),
                ..Site::word(EVENT_NAMESPACE, "actionId", start..end)
            },
        },
    ])
}

fn original_action(
    bank: &Soundbank<&[u8]>,
    path: &'static str,
    id: u32,
    action_type: u16,
    target: u32,
) -> Result<[RouteSite<'static>; 2], Box<dyn Error>> {
    let action = bank.object(id).ok_or("original Action")?;
    let body = bank.object_body(action);
    let kind = action.id_byte_range().end;
    let target_start = kind + size_of::<u16>();
    let target_end = target_start + ID_BYTES;
    assert_eq!(
        body.get(ID_BYTES..ID_BYTES + size_of::<u16>()),
        Some(action_type.to_le_bytes().as_slice()),
        "the original action type"
    );
    assert_eq!(
        body.get(ID_BYTES + size_of::<u16>()..ID_BYTES + size_of::<u16>() + ID_BYTES),
        Some(target.to_le_bytes().as_slice()),
        "the original action target"
    );
    Ok([
        RouteSite {
            path,
            site: Site::word(ACTION_NAMESPACE, "actionType", kind..target_start),
        },
        RouteSite {
            path,
            site: Site::word(ACTION_NAMESPACE, "targetId", target_start..target_end),
        },
    ])
}

fn fixture(root: &[u8], relay: &[u8], choice: &[u8]) -> Result<Fixture, Box<dyn Error>> {
    let directory = tempfile::tempdir()?;
    let archive = directory.path().join("Data.p4k");
    let out = directory.path().join("out");
    let definitions = TestAtl::new("SC_Audio").trigger(EVENT, &[EVENT]);
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data/Libs/GameAudio/SC_Audio.xml",
            definitions.to_cryxml(),
        ))
        .entry(TestEntry::zstd(ROOT_PATH, root).encrypted())
        .entry(TestEntry::stored(RELAY_PATH, relay))
        .entry(TestEntry::stored(BANK_PATH, choice))
        .write_to(&archive)?;
    Ok(Fixture {
        _dir: directory,
        archive,
        out,
    })
}

fn root_bank() -> Vec<u8> {
    TestBank::new(wwise_short_id(ROOT_NAME))
        .version(154)
        .wem(SIBLING_WEM, pcm())
        .object(TestHircObject::event(wwise_short_id(EVENT), &ROOT_ACTIONS))
        .object(TestHircObject::action(
            POST_ACTION,
            POST_EVENT,
            POSTED_EVENT,
        ))
        .object(TestHircObject::action(SIBLING_ACTION, PLAY, SIBLING_SOUND))
        .object(TestHircObject::sound(
            SIBLING_SOUND,
            SIBLING_WEM,
            EMBEDDED,
            0,
        ))
        .bytes()
}

fn relay_bank() -> Vec<u8> {
    TestBank::new(wwise_short_id(RELAY_NAME))
        .version(154)
        .object(TestHircObject::event(POSTED_EVENT, &POSTED_ACTIONS))
        .object(TestHircObject::action_in_bank(
            PLAY_ACTION,
            PLAY,
            CONTAINER,
            wwise_short_id(CHOICE_BANK),
        ))
        .bytes()
}

fn choice_bank() -> Vec<u8> {
    TestBank::new(wwise_short_id(CHOICE_BANK))
        .version(154)
        .wem(FIRST_WEM, pcm())
        .wem(SECOND_WEM, pcm())
        .object(TestHircObject::container(
            TestContainerKind::Random,
            CONTAINER,
            &[FIRST_SOUND, SECOND_SOUND],
            TestNodeBase::Plain,
        ))
        .object(TestHircObject::sound(FIRST_SOUND, FIRST_WEM, EMBEDDED, 0))
        .object(TestHircObject::sound(SECOND_SOUND, SECOND_WEM, EMBEDDED, 0))
        .bytes()
}
