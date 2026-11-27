// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;
use std::io::{self, Cursor, Write};
use std::path::Path;

use p4kx_archive::{Archive, Entry, SourceEncryption};
use p4kx_audio::{BankHircSemantic, HircKind};
use p4kx_testkit::{real_archive_b_path, real_archive_key, real_archive_path};

use crate::hirc_view::ObjectView;
use crate::sound;
use crate::{ArchiveWatch, Build};

use super::{Bank, BankReadOrigin, MAX_SOUNDBANK_LEN, SourceProtection};

#[path = "bank_decision_witness_containers.rs"]
mod containers;
#[path = "bank_decision_witness_dialogue.rs"]
mod dialogue;
#[path = "bank_decision_witness_music.rs"]
mod music;
#[path = "bank_decision_witness_synthetic.rs"]
mod synthetic;
#[path = "bank_decision_witness_values.rs"]
mod values;

use self::values::Projection;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
type Input<'bank> = Cursor<&'bank [u8]>;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Family {
    RandomSequence,
    Switch,
    Blend,
    TrackAutomation,
    TrackTransition,
    SegmentStinger,
    PlaylistStinger,
    SwitchStinger,
    PlaylistTransition,
    SwitchTransition,
    MusicDecision,
    Dialogue,
}

impl Family {
    const ALL: [Self; 12] = [
        Self::RandomSequence,
        Self::Switch,
        Self::Blend,
        Self::TrackAutomation,
        Self::TrackTransition,
        Self::SegmentStinger,
        Self::PlaylistStinger,
        Self::SwitchStinger,
        Self::PlaylistTransition,
        Self::SwitchTransition,
        Self::MusicDecision,
        Self::Dialogue,
    ];

    const fn owner(self) -> HircKind {
        match self {
            Self::RandomSequence => HircKind::RandomSequenceContainer,
            Self::Switch => HircKind::SwitchContainer,
            Self::Blend => HircKind::LayerContainer,
            Self::TrackAutomation | Self::TrackTransition => HircKind::MusicTrack,
            Self::SegmentStinger => HircKind::MusicSegment,
            Self::PlaylistStinger | Self::PlaylistTransition => HircKind::MusicPlaylist,
            Self::SwitchStinger | Self::SwitchTransition | Self::MusicDecision => {
                HircKind::MusicSwitch
            }
            Self::Dialogue => HircKind::DialogueEvent,
        }
    }
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn populated_wwise_decision_families_match_original_bytes_in_memory() -> TestResult {
    verify(&real_archive_path()?)?;
    if let Some(path) = real_archive_b_path() {
        verify(&path)?;
    }
    Ok(())
}

fn verify(path: &Path) -> TestResult {
    let archive = Archive::open_with_key(path, Some(real_archive_key()?))
        .map_err(|_| "open sound Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive.build().ok_or("authoritative installed Build")?;
    let mut output = io::stdout().lock();
    writeln!(
        output,
        "Wwise Build {} {} {} {}",
        build.branch(),
        build.version(),
        build.change_number(),
        build.build_id()
    )?;
    output.flush()?;
    let inventory = inspect_archive(&archive, &watch)?;
    let folder = path.parent().ok_or("installed channel folder")?;
    require(
        Build::read_from(folder)
            .map_err(|_| "installed Build manifest")?
            .as_ref()
            == Some(build),
        "unchanged installed Build",
    )?;
    writeln!(
        output,
        "Wwise root BNKs: verified v154 {}, verified other versions {}, above supported bank size {}",
        inventory.compared_banks, inventory.other_versions, inventory.skipped_size
    )?;
    let scope = if inventory.covered() {
        "required populated families witnessed"
    } else {
        "root BNK candidates exhausted before populated coverage"
    };
    writeln!(output, "Wwise witness scope: {scope}")?;
    inventory.report(&mut output)?;
    output.flush()?;
    require(
        inventory.covered(),
        "required populated Wwise decisions witnessed",
    )
}

struct Inventory {
    missing: BTreeSet<Family>,
    compared: BTreeSet<Family>,
    compared_banks: usize,
    other_versions: usize,
    skipped_size: usize,
}

fn inspect_archive(archive: &Archive, watch: &ArchiveWatch) -> TestResult<Inventory> {
    let mut inventory = Inventory::new();
    for entry in archive.entries().iter().filter(|entry| is_bank(entry)) {
        watch.poll().map_err(|_| "unchanged sound Archive")?;
        let Some(bank) = read_bank(archive, entry)? else {
            inventory.skipped_size += 1;
            continue;
        };
        if bank.soundbank().header().version == 154 {
            inventory.compared_banks += 1;
            observe(&bank, watch, &mut inventory)?;
            if inventory.covered() {
                break;
            }
        } else {
            inventory.other_versions += 1;
        }
    }
    watch.check().map_err(|_| "unchanged sound Archive")?;
    Ok(inventory)
}

impl Inventory {
    fn new() -> Self {
        Self {
            missing: BTreeSet::from(Family::ALL),
            compared: BTreeSet::new(),
            compared_banks: 0,
            other_versions: 0,
            skipped_size: 0,
        }
    }

    fn covered(&self) -> bool {
        use Family::{
            Blend, Dialogue, MusicDecision, PlaylistStinger, PlaylistTransition, RandomSequence,
            SegmentStinger, Switch, SwitchStinger, SwitchTransition, TrackAutomation,
            TrackTransition,
        };
        let witnessed = |family: Family| !self.missing.contains(&family);
        [
            RandomSequence,
            Switch,
            Blend,
            TrackAutomation,
            TrackTransition,
            MusicDecision,
            Dialogue,
        ]
        .into_iter()
        .all(witnessed)
            && [SegmentStinger, PlaylistStinger, SwitchStinger]
                .into_iter()
                .any(witnessed)
            && [PlaylistTransition, SwitchTransition]
                .into_iter()
                .any(witnessed)
    }

    fn report(&self, output: &mut impl Write) -> io::Result<()> {
        for family in Family::ALL {
            let status = if !self.missing.contains(&family) {
                "witnessed"
            } else if self.compared.contains(&family) {
                "compared without populated evidence"
            } else {
                "outside inspected coverage"
            };
            writeln!(output, "{family:?}: {status}")?;
        }
        Ok(())
    }
}

fn is_bank(entry: &Entry) -> bool {
    entry
        .path()
        .rsplit_once('.')
        .is_some_and(|(_, extension)| extension.eq_ignore_ascii_case("bnk"))
}

fn read_bank(archive: &Archive, entry: &Entry) -> TestResult<Option<Bank>> {
    if entry.uncompressed_size() > MAX_SOUNDBANK_LEN {
        return Ok(None);
    }
    let origin = archive
        .entry_origin(entry)
        .ok_or("original BNK Entry identity")?;
    let reader = archive
        .open_entry_verified(entry)
        .map_err(|_| "open verified original BNK")?;
    let bytes = sound::read_capped(reader, entry.uncompressed_size(), MAX_SOUNDBANK_LEN)
        .map_err(|_| "verify original BNK checksum")?
        .ok_or("original BNK within supported size")?;
    let bank = Bank::parse(
        entry.path().to_owned(),
        Some(bytes),
        SourceProtection::new(Some(archive.path())),
        BankReadOrigin::Archive(origin),
    )
    .map_err(|_| "parse verified original BNK")?;
    Ok(Some(bank))
}

fn observe(bank: &Bank, watch: &ArchiveWatch, inventory: &mut Inventory) -> TestResult {
    if bank.soundbank().header().version != 154 {
        return Ok(());
    }
    let source = bank.archive_origin().ok_or("observed bank source")?;
    let protected = match source.encryption() {
        SourceEncryption::Encrypted => Some(true),
        SourceEncryption::Unencrypted => Some(false),
        _ => None,
    };
    for (index, owner) in bank.soundbank().objects().iter().enumerate() {
        if !inventory
            .missing
            .iter()
            .any(|family| family.owner() == owner.kind)
        {
            continue;
        }
        watch.poll().map_err(|_| "unchanged sound Archive")?;
        let mut view = ObjectView::new(index, owner);
        view.attach_source_fields(bank);
        let fields = view
            .source_fields
            .as_ref()
            .ok_or("source view at the original object ordinal")?;
        let document =
            serde_json::to_value(fields).map_err(|_| "serialize original sound object")?;
        let comparison = bank
            .soundbank()
            .with_hirc_source_fields(index, |_, semantic| {
                let body = bank.soundbank().object_body(owner);
                let projection = Projection::new(
                    &document,
                    body,
                    semantic.object().body_offset(),
                    protected,
                    source.entry().ordinal(),
                )?;
                compare(owner.kind, semantic, &projection, &mut inventory.missing)
            })
            .map_err(|_| "replay the physical sound object")?
            .ok_or("physical sound object at selected ordinal")?;
        comparison.map_err(|error| format!("{}: {error}", owner.kind.slug()))?;
        inventory.compared.extend(
            Family::ALL
                .into_iter()
                .filter(|family| family.owner() == owner.kind),
        );
        if inventory.covered() {
            break;
        }
    }
    Ok(())
}

fn compare(
    kind: HircKind,
    semantic: BankHircSemantic<'_, Input<'_>>,
    projection: &Projection<'_>,
    missing: &mut BTreeSet<Family>,
) -> TestResult {
    match semantic {
        BankHircSemantic::ContainerWithNode(mut node) => {
            let observed = match kind {
                HircKind::RandomSequenceContainer => {
                    containers::random_sequence(&mut node, projection)?
                        .then_some(Family::RandomSequence)
                }
                HircKind::SwitchContainer => {
                    containers::switch(&mut node, projection)?.then_some(Family::Switch)
                }
                HircKind::LayerContainer => {
                    containers::blend(&mut node, projection)?.then_some(Family::Blend)
                }
                _ => None,
            };
            if let Some(family) = observed {
                missing.remove(&family);
            }
        }
        BankHircSemantic::DialogueEvent(mut node) => {
            if dialogue::dialogue(&mut node, projection)? {
                missing.remove(&Family::Dialogue);
            }
        }
        BankHircSemantic::MusicTrack(mut node) => {
            music::track(&mut node, projection, missing)?;
        }
        BankHircSemantic::MusicSegment(node) => {
            music::segment(*node, projection, missing)?;
        }
        BankHircSemantic::MusicPlaylist(mut node) => {
            music::playlist(&mut node, projection, missing)?;
        }
        BankHircSemantic::MusicSwitch(mut node) => {
            music::switch(&mut node, projection, missing)?;
        }
        _ => return Err("requested Wwise-154 family lacks a typed source reader".into()),
    }
    Ok(())
}

fn require(condition: bool, description: &'static str) -> TestResult {
    if condition {
        Ok(())
    } else {
        Err(description.into())
    }
}
