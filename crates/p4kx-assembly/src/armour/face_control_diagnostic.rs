// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_character::RigLogic;
use p4kx_model::{Animation, Track};
use serde_json::{Value, json};

use super::super::super::ArmourPart;
use super::super::super::facial::{FaceLink, FaceSource};
use super::super::super::sample::values_of;
use super::super::FaceTracks;

#[derive(Clone, Copy)]
enum ControlGroup {
    Usable,
    Constant,
    MissingTranslation,
    Unknown,
}

impl ControlGroup {
    const fn code(self) -> &'static str {
        match self {
            Self::Usable => "usableControl",
            Self::Constant => "constantControl",
            Self::MissingTranslation => "missingTranslation",
            Self::Unknown => "unknownController",
        }
    }
}

#[derive(Default)]
struct Counts {
    usable: usize,
    constant: usize,
    missing_translation: usize,
    unknown: usize,
}

impl Counts {
    const fn observe(&mut self, group: ControlGroup) {
        match group {
            ControlGroup::Usable => self.usable += 1,
            ControlGroup::Constant => self.constant += 1,
            ControlGroup::MissingTranslation => self.missing_translation += 1,
            ControlGroup::Unknown => self.unknown += 1,
        }
    }

    fn record(&self) -> Value {
        json!({
            "usableControl": self.usable,
            "constantControl": self.constant,
            "missingTranslation": self.missing_translation,
            "unknownController": self.unknown,
        })
    }
}

const fn classify(control: Option<usize>, has_translation: bool) -> ControlGroup {
    match (control, has_translation) {
        (None, _) => ControlGroup::Unknown,
        (Some(_), false) => ControlGroup::MissingTranslation,
        (Some(0), true) => ControlGroup::Constant,
        (Some(_), true) => ControlGroup::Usable,
    }
}

fn activity(values: &[[f32; 3]]) -> &'static str {
    let Some([first, ..]) = values.first() else {
        return "empty";
    };
    if values.iter().any(|[value, ..]| !value.is_finite()) {
        "nonFinite"
    } else if values.iter().all(|[value, ..]| *value == 0.0) {
        "zero"
    } else if values
        .iter()
        .all(|[value, ..]| value.to_bits() == first.to_bits())
    {
        "constantNonzero"
    } else {
        "varying"
    }
}

fn translations(track: &Track) -> Value {
    let keys = track.translations.as_ref();
    json!({
        "present": keys.is_some(),
        "frames": keys.map_or(0, |keys| keys.frames.len()),
        "values": keys.map_or(0, |keys| keys.values.len()),
        "firstChannelActivity": keys.map(|keys| activity(&keys.values)),
    })
}

fn source(source: &FaceSource) -> Value {
    match source {
        FaceSource::Dna { rig, .. } => json!({"kind": "dna", "path": rig}),
        FaceSource::Own => json!({"kind": "own", "path": null}),
    }
}

pub(super) fn failure(
    animation: &Animation,
    (part_index, part): (usize, &ArmourPart),
    link: &FaceLink,
    logic: &RigLogic,
    selected: &FaceTracks<'_>,
) -> String {
    let mut counts = Counts::default();
    let tracks: Vec<_> = animation
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| {
            let control = logic.control_by_hash(track.controller_id);
            let group = classify(control, values_of(track).is_some());
            counts.observe(group);
            json!({
                "track": index,
                "controllerId": track.controller_id,
                "group": group.code(),
                "control": control,
                "controlName": control.and_then(|index| logic.name(index)),
                "translation": translations(track),
                "rotationFrames": track.rotations.as_ref().map_or(0, |keys| keys.frames.len()),
                "rotationValues": track.rotations.as_ref().map_or(0, |keys| keys.values.len()),
            })
        })
        .collect();
    let report = json!({
        "record": "faceControlBindingFailure",
        "part": {"index": part_index, "name": part.name(), "file": part.file()},
        "rigSource": source(&link.source),
        "rigControls": logic.controls(),
        "rigBlendshapes": logic.blendshapes(),
        "selection": {
            "usableControls": selected.controls.len(),
            "drivenTracks": selected.driven.len(),
            "skippedTracks": selected.skipped.len(),
        },
        "groups": counts.record(),
        "tracks": tracks,
    });
    format!(
        "comparison needs a completely bound facial control sequence; controllerBinding={report}"
    )
}
