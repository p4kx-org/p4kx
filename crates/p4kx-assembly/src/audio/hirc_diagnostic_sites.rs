// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_audio::{Container, ContainerLoopEvidence, ContainerSwitchEvidence, HircValue};

use super::txtp::TxtpApproximation;

mod music;
mod track;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiagnosticWord {
    pub(super) namespace: &'static str,
    pub(super) field: &'static str,
    pub(super) bytes: Range<u64>,
}

impl DiagnosticWord {
    fn new(namespace: &'static str, field: &'static str, bytes: &Range<u64>) -> Self {
        Self {
            namespace,
            field,
            bytes: bytes.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DiagnosticSite {
    pub(super) approximation: TxtpApproximation,
    first: DiagnosticWord,
    second: Option<DiagnosticWord>,
}

impl DiagnosticSite {
    const fn one(approximation: TxtpApproximation, word: DiagnosticWord) -> Self {
        Self {
            approximation,
            first: word,
            second: None,
        }
    }

    const fn with_word(mut self, word: DiagnosticWord) -> Self {
        self.second = Some(word);
        self
    }

    pub(super) fn words(&self) -> impl Iterator<Item = &DiagnosticWord> {
        std::iter::once(&self.first).chain(self.second.as_ref())
    }
}

pub(super) fn checked_sites(value: &HircValue) -> Vec<DiagnosticSite> {
    let mut sites = Vec::new();
    match value {
        HircValue::Container(container) => container_sites(&mut sites, container),
        HircValue::MusicTrack(track) => track::append(&mut sites, track),
        HircValue::MusicPlaylist(playlist) => music::playlist_sites(&mut sites, playlist),
        HircValue::MusicSwitch(switch) => music::switch_sites(&mut sites, switch),
        _ => {}
    }
    sites
}

fn container_sites(sites: &mut Vec<DiagnosticSite>, container: &Container) {
    const RANDOM_SEQUENCE: &str = "bnkHircRandomSequence";
    const SWITCH: &str = "bnkHircSwitchContainer";

    if let Some(mode) = container.transition_mode_evidence() {
        sites.push(DiagnosticSite::one(
            TxtpApproximation::OmitContainerTransitions,
            DiagnosticWord::new(RANDOM_SEQUENCE, "randomSequenceTransitionMode", &mode.range),
        ));
    }
    if let Some(points) = container.blend_point_count_evidence() {
        sites.push(DiagnosticSite::one(
            TxtpApproximation::OmitBlendCurves,
            DiagnosticWord::new("bnkHircBlendContainer", "blendPointCount", &points.range),
        ));
    }
    if let Some(word) = container.loop_evidence().and_then(container_loop) {
        sites.push(DiagnosticSite::one(
            TxtpApproximation::OmitContainerLoops,
            word,
        ));
    }
    let condition = match container.switch_condition_evidence() {
        Some(ContainerSwitchEvidence::DefaultValue(field)) => {
            DiagnosticWord::new(SWITCH, "switchDefaultValueId", &field.range)
        }
        Some(ContainerSwitchEvidence::MembershipGroupCount(field)) => {
            DiagnosticWord::new(SWITCH, "switchMembershipGroupCount", &field.range)
        }
        _ => return,
    };
    sites.push(DiagnosticSite::one(
        TxtpApproximation::OmitContainerConditions,
        condition,
    ));
}

fn container_loop(evidence: &ContainerLoopEvidence) -> Option<DiagnosticWord> {
    const NAMESPACE: &str = "bnkHircRandomSequence";
    let (name, field) = match evidence {
        ContainerLoopEvidence::Count(field) => ("randomSequenceLoopCount", field),
        ContainerLoopEvidence::Minimum(field) => ("randomSequenceLoopMinimum", field),
        ContainerLoopEvidence::Maximum(field) => ("randomSequenceLoopMaximum", field),
        _ => return None,
    };
    Some(DiagnosticWord::new(NAMESPACE, name, &field.range))
}
