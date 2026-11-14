// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Color32;
use fluent_bundle::FluentValue;

use super::model_export_animations as animations;
use crate::theme::palette;
use crate::{MaterialProblem, ModelExportSummary, ModelExportText as T, Texts, format};

/// Returns the heading and the lines of the textures the file lacks.
pub(super) fn missing_textures(
    texts: &Texts,
    summary: &ModelExportSummary,
) -> (String, Vec<String>) {
    let heading = texts.format(
        T::SummaryTextures,
        &[(
            "number",
            number(texts, summary.missing_textures().len()).into(),
        )],
    );
    let lines = summary
        .missing_textures()
        .iter()
        .map(|texture| {
            texts.format(
                T::SummaryTexture,
                &[
                    ("material", texture.material.clone().into()),
                    ("role", texture.role.name().into()),
                    ("file", texture.file.clone().into()),
                    ("reason", texture.reason.clone().into()),
                ],
            )
        })
        .collect();
    (heading, lines)
}

pub(super) fn number(texts: &Texts, value: usize) -> String {
    format::number(u64::try_from(value).unwrap_or(u64::MAX), texts.language())
}

/// Returns the lines above the list of a summary, each with its colour.
pub(super) fn notes(texts: &Texts, summary: &ModelExportSummary) -> Vec<(String, Color32)> {
    let written = summary.written();
    let count = |value: usize| FluentValue::from(number(texts, value));
    let mut notes = vec![(
        texts.format(
            T::SummaryFile,
            &[("path", summary.file().display().to_string().into())],
        ),
        palette::GRAY_1,
    )];
    let counts = [
        ("nodes", count(written.nodes)),
        ("meshes", count(written.meshes)),
        ("materials", count(written.materials)),
        ("vertices", count(written.vertices)),
        ("triangles", count(written.triangles)),
        ("size", format::size(written.bytes, texts.language()).into()),
    ];
    notes.push((texts.format(T::SummaryCounts, &counts), palette::GRAY_1));
    if written.bones > 0 {
        let args = [
            ("bones", count(written.bones)),
            ("skinned", count(written.skinned_meshes)),
        ];
        notes.push((texts.format(T::SummaryBones, &args), palette::GRAY_1));
    }
    if let Some(line) = animations::written_line(texts, summary) {
        notes.push((line, palette::GRAY_1));
    }
    if let Some(problem) = summary.material_problem() {
        notes.push((library_note(texts, problem), palette::WARNING));
    } else {
        counted(
            texts,
            &mut notes,
            (T::SummaryDefaultMaterials, written.default_material_ids),
            palette::WARNING,
        );
    }
    counted(
        texts,
        &mut notes,
        (T::SummaryUnnamedMaterials, summary.unnamed_sub_materials()),
        palette::WARNING,
    );
    counted(
        texts,
        &mut notes,
        (T::SummaryMissingLayers, summary.missing_layers()),
        palette::WARNING,
    );
    counted(
        texts,
        &mut notes,
        (T::SummaryHidden, written.hidden_primitives),
        palette::GRAY_1,
    );
    notes
}

fn library_note(texts: &Texts, problem: &MaterialProblem) -> String {
    match problem {
        MaterialProblem::Missing {
            library,
            target: None,
        } => texts.format(
            T::SummaryLibraryMissing,
            &[("library", library.as_str().into())],
        ),
        MaterialProblem::Missing {
            library,
            target: Some(target),
        } => texts.format(
            T::SummaryLibraryMissingMapped,
            &[
                ("library", library.as_str().into()),
                ("target", target.as_str().into()),
            ],
        ),
        MaterialProblem::Unreadable {
            library,
            target: None,
            reason,
        } => texts.format(
            T::SummaryLibraryUnreadable,
            &[
                ("library", library.as_str().into()),
                ("reason", reason.as_str().into()),
            ],
        ),
        MaterialProblem::Unreadable {
            library,
            target: Some(target),
            reason,
        } => texts.format(
            T::SummaryLibraryUnreadableMapped,
            &[
                ("library", library.as_str().into()),
                ("target", target.as_str().into()),
                ("reason", reason.as_str().into()),
            ],
        ),
    }
}

/// Adds the line `text` for `value` things, unless there are none.
fn counted(
    texts: &Texts,
    notes: &mut Vec<(String, Color32)>,
    (text, value): (T, usize),
    colour: Color32,
) {
    if value == 0 {
        return;
    }
    let args = [
        ("count", u64::try_from(value).unwrap_or(u64::MAX).into()),
        ("number", number(texts, value).into()),
    ];
    notes.push((texts.format(text, &args), colour));
}

#[cfg(test)]
#[path = "model_export_notes_tests.rs"]
mod tests;
