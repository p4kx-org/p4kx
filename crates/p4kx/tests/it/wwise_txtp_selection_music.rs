// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::error::Error;
use std::iter;

use p4kx_extraction::Soundbank;
use p4kx_testkit::{TestBank, TestEntry, TestHircObject, wwise_short_id};

use super::{
    BANK_PATH, EVENT, FIRST_WEM, ID_BYTES, PLAY, SECOND_WEM, Site, TXTP_FILE, TestResult,
    check_receipt, export, fixture_with_entries, pcm, referenced_sites, rendered_commands,
    selection_field,
};

const SWITCH: u32 = 30;
const PLAYLIST: u32 = 0x7100_0100;
const FIRST_SEGMENT: u32 = 0x7100_0101;
const SECOND_SEGMENT: u32 = 0x7100_0102;
const DIRECT_SEGMENT: u32 = 0x7100_0103;
const FIRST_TRACK: u32 = 0x7100_0201;
const SECOND_TRACK: u32 = 0x7100_0202;
const DIRECT_TRACK: u32 = 0x7100_0203;
const DIRECT_WEM: u32 = 102;
const SWITCH_GROUP: u32 = 0x42;
const SWITCH_CHILDREN: [u32; 2] = [PLAYLIST, DIRECT_SEGMENT];
const PLAYLIST_CHILDREN: [u32; 2] = [FIRST_SEGMENT, SECOND_SEGMENT];
const TRACKS: [(u32, u32, u32); 3] = [
    (FIRST_SEGMENT, FIRST_TRACK, FIRST_WEM),
    (SECOND_SEGMENT, SECOND_TRACK, SECOND_WEM),
    (DIRECT_SEGMENT, DIRECT_TRACK, DIRECT_WEM),
];
const MUSIC: &str = "bnkHircMusicNode";

#[test]
fn selection_attests_to_music_switch_playlist_and_segment_child_words() -> TestResult {
    let bank = bank();
    let loose = TRACKS
        .map(|(_, _, id)| TestEntry::stored(format!("Data/Sounds/wwise/Media/{id}.wem"), pcm()));
    let fixture = fixture_with_entries(
        iter::once(TestEntry::stored(BANK_PATH, bank.as_slice())).chain(loose),
    )?;
    let report = export(&fixture, &[])?;
    let file = report.pointer(TXTP_FILE).ok_or("written music TXTP file")?;
    let field = selection_field(file)?;
    let origin = file["fieldOrigins"]
        .get(&field)
        .ok_or("music selection origin")?;
    let text = check_receipt(&fixture.out, file, origin)?;
    let commands: Vec<_> = rendered_commands(&text).collect();
    assert_eq!(
        commands,
        [
            "Play_Topology_100.wav",
            "Play_Topology_101.wav",
            "group = -S2",
            "Play_Topology_102.wav",
            "group = -R2>1",
        ],
        "the playlist is one of the two MusicSwitch alternatives"
    );
    assert_eq!(
        referenced_sites(file, origin)?,
        original_sites(&bank)?,
        "the choice cites the original lists, including collapsed MusicSegment children"
    );
    Ok(())
}

fn original_sites(bytes: &[u8]) -> Result<BTreeSet<Site<'static>>, Box<dyn Error>> {
    let bank = Soundbank::parse(bytes)?;
    let version = bank.version_byte_range().ok_or("music BKHD version")?;
    let mut sites = BTreeSet::from([Site::word("bnkBkhd", "version", version)]);
    for (owner, children) in [
        (SWITCH, SWITCH_CHILDREN.as_slice()),
        (PLAYLIST, PLAYLIST_CHILDREN.as_slice()),
    ] {
        insert_music_sites(&bank, owner, children, &mut sites)?;
    }
    for (segment, track, _) in TRACKS {
        insert_music_sites(&bank, segment, &[track], &mut sites)?;
    }
    Ok(sites)
}

fn insert_music_sites(
    bank: &Soundbank<&[u8]>,
    owner: u32,
    children: &[u32],
    sites: &mut BTreeSet<Site<'static>>,
) -> TestResult {
    let container = bank.object(owner).ok_or("original music container")?;
    let kind = container.byte_range().start;
    sites.insert(Site::word("bnkHircObject", "typeCode", kind..kind + 1));
    let count_word = u32::try_from(children.len())?;
    let words: Vec<_> = iter::once(count_word)
        .chain(children.iter().copied())
        .flat_map(u32::to_le_bytes)
        .collect();
    let mut positions = bank
        .object_body(container)
        .windows(words.len())
        .enumerate()
        .filter(|(_, window)| *window == words.as_slice());
    let (offset, _) = positions.next().ok_or("authored music child list")?;
    assert!(
        positions.next().is_none(),
        "one physical child list for owner {owner}"
    );
    let count = container.id_byte_range().start + offset;
    let first = count + ID_BYTES;
    sites.insert(Site::word(MUSIC, "musicChildCount", count..first));
    for ordinal in 0..children.len() {
        let start = first + ordinal * ID_BYTES;
        sites.insert(Site {
            ordinal: Some(ordinal),
            ..Site::word(MUSIC, "musicChildId", start..start + ID_BYTES)
        });
    }
    Ok(())
}

fn bank() -> Vec<u8> {
    let mut bank = TestBank::new(wwise_short_id("Topology"))
        .version(154)
        .object(TestHircObject::event(wwise_short_id(EVENT), &[1]))
        .object(TestHircObject::action(1, PLAY, SWITCH))
        .object(TestHircObject::music_switch(
            SWITCH,
            &SWITCH_CHILDREN,
            SWITCH_GROUP,
            &SWITCH_CHILDREN,
        ))
        .object(TestHircObject::music_playlist(PLAYLIST, &PLAYLIST_CHILDREN));
    for (segment, track, wem) in TRACKS {
        bank = bank
            .object(TestHircObject::music_segment(segment, &[track], &[]))
            .object(TestHircObject::music_track(track, &[wem]));
    }
    bank.bytes()
}
