// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{AudioIndex, WemLocation};
use p4kx_extraction::{Archive, Bank, EmbeddedWem, Sound};
use p4kx_testkit::{TestBank, TestCodec, TestEntry, TestWem};

use crate::support::{Install, TestResult};

pub(crate) const ID: u32 = 7819;
pub(crate) const DIRECT: &str = "Data\\Sounds\\wwise\\named-control.wem";
pub(crate) const SAMPLES: [i16; 8] = [0, 8000, -8000, i16::MAX, i16::MIN, 7, -7, 0];

#[derive(Clone, Copy)]
enum Kind {
    Prefetch,
    Bad,
    Full,
}

#[derive(Clone, Copy)]
pub(crate) enum Scenario {
    PrefetchThenFull,
    ErrorsThenFull,
    OnlyPrefetch,
    MixedNoFull,
    FirstFull,
}

pub(crate) struct Fixture {
    pub(crate) install: Install,
    pub(crate) original: Original,
}

pub(crate) struct Original {
    pub(crate) full: Option<String>,
    pub(crate) first_error: Option<Failure>,
}

pub(crate) struct Failure {
    pub(crate) path: String,
    pub(crate) reason: String,
}

pub(crate) fn bank_path(ordinal: usize) -> String {
    format!("Data\\Sounds\\wwise\\bank{ordinal:02}.bnk")
}

pub(crate) fn create(scenario: Scenario) -> TestResult<Fixture> {
    let kinds = kinds(scenario);
    let mut entries = Vec::new();
    for (index, kind) in kinds.iter().enumerate() {
        let bytes = TestBank::new(u32::try_from(index + 1)?)
            .wem(ID, media(*kind)?)
            .bytes();
        entries.push(TestEntry::stored(bank_path(index + 1), bytes));
    }
    entries.push(TestEntry::stored(DIRECT, media(Kind::Full)?));
    let install = Install::new(entries)?;
    let archive = Archive::open_with_key(install.archive(), None)?;
    original_index(&archive)?;
    let original = original_banks(&archive, &kinds)?;
    let direct = Sound::from_archive(&archive, DIRECT)?;
    assert_eq!(
        direct.wem().sample_count(),
        Some(8),
        "the explicit-path control is actual original PCM"
    );
    Ok(Fixture { install, original })
}

fn kinds(scenario: Scenario) -> [Kind; 9] {
    let mut kinds = [Kind::Prefetch; 9];
    match scenario {
        Scenario::PrefetchThenFull => kinds[8] = Kind::Full,
        Scenario::ErrorsThenFull => {
            kinds.fill(Kind::Bad);
            kinds[8] = Kind::Full;
        }
        Scenario::OnlyPrefetch => {}
        Scenario::MixedNoFull => kinds[1] = Kind::Bad,
        Scenario::FirstFull => kinds[0] = Kind::Full,
    }
    kinds
}

fn media(kind: Kind) -> TestResult<Vec<u8>> {
    let full = TestWem::pcm(TestCodec::Pcm, 1, 8000, &SAMPLES).bytes();
    assert!(
        full.ends_with(&pcm()),
        "the builder's raw original sample words are the literal oracle"
    );
    Ok(match kind {
        Kind::Full => full,
        Kind::Prefetch => full
            .get(..40)
            .ok_or("actual RIFF prefetch prefix")?
            .to_vec(),
        Kind::Bad => b"not a WEM".to_vec(),
    })
}

pub(crate) fn pcm() -> Vec<u8> {
    SAMPLES
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect()
}

fn original_index(archive: &Archive) -> TestResult {
    let index = AudioIndex::build(archive);
    assert_eq!(
        index.loose_wem(ID).count(),
        0,
        "no loose same-ID sound bypasses the nine-bank path"
    );
    let mut paths = Vec::new();
    for place in index.embedded_wem(ID) {
        if let WemLocation::Soundbank { soundbank, .. } = place {
            paths.push(soundbank.path().to_owned());
        } else {
            return Err("the actual nine candidates are embedded banks".into());
        }
    }
    let expected: Vec<_> = (1..=9).map(bank_path).collect();
    assert_eq!(
        paths, expected,
        "all nine original indexed locations are in actual source order"
    );
    Ok(())
}

fn original_banks(archive: &Archive, kinds: &[Kind; 9]) -> TestResult<Original> {
    let mut original = Original {
        full: None,
        first_error: None,
    };
    for (index, kind) in kinds.iter().enumerate() {
        let path = bank_path(index + 1);
        let bank = Bank::from_archive(archive, &path)?;
        let wem = bank
            .soundbank()
            .wem(ID)
            .ok_or("the actual embedded WEM occurrence")?;
        if let Some(reason) = original_media(&wem, *kind)? {
            original.first_error.get_or_insert(Failure { path, reason });
        } else if matches!(kind, Kind::Full) {
            original.full.get_or_insert(path);
        }
    }
    Ok(original)
}

fn original_media(wem: &EmbeddedWem<'_>, kind: Kind) -> TestResult<Option<String>> {
    match kind {
        Kind::Full => {
            assert!(
                !wem.is_prefetch(),
                "the actual complete candidate is not prefetched"
            );
            let parsed = wem.parse()?;
            assert_eq!(parsed.codec().id(), "pcm", "actual original readable codec");
            assert_eq!(
                parsed.sample_count(),
                Some(8),
                "actual original complete frame count"
            );
            assert!(
                wem.bytes().ends_with(&pcm()),
                "the actual Archive bytes contain the exact sample oracle"
            );
        }
        Kind::Prefetch => {
            assert!(
                wem.is_prefetch(),
                "every intended original incomplete location really is prefetch"
            );
            assert!(
                wem.parse().is_err(),
                "the declared RIFF length exceeds the retained prefix"
            );
        }
        Kind::Bad => {
            assert!(
                !wem.is_prefetch(),
                "a malformed full candidate is distinct from prefetch"
            );
            let error = wem
                .parse()
                .err()
                .ok_or("the actual original malformed WEM")?;
            return Ok(Some(error.to_string()));
        }
    }
    Ok(None)
}
