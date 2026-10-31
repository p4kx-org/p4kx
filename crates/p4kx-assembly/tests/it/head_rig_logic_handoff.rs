// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shaping a head reads the `RigLogic` chunk the provider handed along with the skin, not a
//! second copy of the skin file.

use std::sync::Mutex;

use p4kx_assembly::{
    ArchiveModels, Armour, AssemblyOptions, BodyVariant, Expressions, IssueKind, ModelFailure,
    ModelProvider, Piece, ProvidedModel,
};
use p4kx_chunks::{ChunkFile, ivo};
use p4kx_extraction::Archive;
use p4kx_extraction::model::MaterialStatus;
use p4kx_testkit::{HEAD_VERTICES, MALE_RIG_HASH, head_entries, head_rig};

use crate::assembly_support::{TestResult, fixture};
use crate::head_build::{dna_of_head_two_in_region_three, head_path};

const SKIN_LIMIT: u64 = 1 << 20;

/// Delegates to `inner`, records the paths of the files it is asked for and, when `strip` is set,
/// hands every model out without its `RigLogic` chunk.
struct Recording<'a> {
    inner: ArchiveModels<'a>,
    files: Mutex<Vec<String>>,
    strip: bool,
}

impl<'a> Recording<'a> {
    const fn new(inner: ArchiveModels<'a>, strip: bool) -> Self {
        Self {
            inner,
            files: Mutex::new(Vec::new()),
            strip,
        }
    }
}

impl ModelProvider for Recording<'_> {
    fn model(&self, path: &str, lod: u32) -> Result<ProvidedModel, ModelFailure> {
        let mut provided = self.inner.model(path, lod)?;
        if self.strip {
            provided.model.rig_logic = None;
        }
        Ok(provided)
    }

    fn library(&self, entry: &str, name: &str) -> MaterialStatus {
        self.inner.library(entry, name)
    }

    fn file(&self, path: &str, limit: u64) -> Result<Vec<u8>, ModelFailure> {
        self.files
            .lock()
            .map_err(|_| ModelFailure::Fatal("the recorded files are poisoned".into()))?
            .push(path.to_owned());
        self.inner.file(path, limit)
    }
}

fn head_armour(models: &Recording<'_>) -> Result<Armour, Box<dyn std::error::Error>> {
    let piece = Piece::file("head", head_path(), None)
        .shaped(dna_of_head_two_in_region_three())
        .with_expressions(Expressions::All);
    Ok(Armour::build(
        BodyVariant::Male,
        &[piece],
        models,
        &AssemblyOptions::default(),
    )?)
}

#[test]
fn the_provider_hands_the_bytes_of_the_rig_logic_chunk_of_the_skin() -> TestResult {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = ArchiveModels::new(&archive);

    let provided = models.model(&head_path(), 0)?;

    let skin = ChunkFile::parse(models.file(&head_path(), SKIN_LIMIT)?)?;
    let (index, _) = skin.chunks_of(ivo::RIG_LOGIC).next().ok_or("the chunk")?;
    let expected = skin.chunk_data(index).ok_or("the chunk data")?;
    let handed = provided
        .model
        .rig_logic
        .as_ref()
        .ok_or("the handed chunk")?;
    assert_eq!(handed.as_slice(), expected);
    Ok(())
}

#[test]
fn shaping_reads_no_second_copy_of_the_skin() -> TestResult {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = Recording::new(ArchiveModels::new(&archive), false);

    let armour = head_armour(&models)?;

    let head = armour
        .parts()
        .iter()
        .find(|part| part.name() == "head")
        .ok_or("the head part")?;
    assert_eq!(head.morphs().len(), 2);
    let files = models
        .files
        .into_inner()
        .map_err(|_| "the recorded files")?;
    assert!(
        !files
            .iter()
            .any(|path| path.eq_ignore_ascii_case(&head_path())),
        "{files:?}"
    );
    Ok(())
}

#[test]
fn the_shaped_head_does_not_carry_the_chunk() -> TestResult {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = Recording::new(ArchiveModels::new(&archive), false);

    let armour = head_armour(&models)?;

    let head = armour
        .parts()
        .iter()
        .find(|part| part.name() == "head")
        .ok_or("the head part")?;
    assert_eq!(head.model().rig_logic, None);
    Ok(())
}

#[test]
fn a_skin_without_the_chunk_fails_to_shape_and_says_so() -> TestResult {
    let fixture = fixture(head_entries(Some(
        head_rig(HEAD_VERTICES, MALE_RIG_HASH).build(),
    )))?;
    let archive = Archive::open(&fixture.path)?;
    let models = Recording::new(ArchiveModels::new(&archive), true);

    let armour = head_armour(&models)?;

    let reasons: Vec<String> = armour
        .issues()
        .iter()
        .filter_map(|issue| match issue.kind() {
            IssueKind::MorphsFailed { reason } | IssueKind::ShapeFailed { reason } => {
                Some(reason.clone())
            }
            _ => None,
        })
        .collect();
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("has no RigLogic chunk")),
        "{:?}",
        armour.issues()
    );
    Ok(())
}
