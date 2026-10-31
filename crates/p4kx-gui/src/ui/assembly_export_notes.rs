// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::Color32;
use fluent_bundle::FluentValue;
use p4kx_blender::PackageSummary;

use super::assembly_export_outcome::number;
use crate::theme::palette;
use crate::{AssemblyExportSummary, AssemblyExportText as T, Texts, format};

/// Returns the lines above the report, each with its colour.
pub(super) fn notes(texts: &Texts, summary: &AssemblyExportSummary) -> Vec<(String, Color32)> {
    let mut notes = file_notes(texts, summary);
    notes.extend(summary.mismatches().iter().map(|mismatch| {
        let args = [
            ("entity", mismatch.entity.clone().into()),
            ("port", mismatch.path.clone().into()),
            ("reason", mismatch.reason.clone().into()),
        ];
        (texts.format(T::SummaryMismatch, &args), palette::WARNING)
    }));
    notes
}

fn file_notes(texts: &Texts, summary: &AssemblyExportSummary) -> Vec<(String, Color32)> {
    if let Some(package) = summary.package() {
        return package_notes(texts, summary, package);
    }
    let written = summary.written();
    let counts = [
        ("parts", number(texts, summary.parts())),
        ("modelled", number(texts, summary.modelled_parts())),
        ("nodes", number(texts, written.nodes)),
        ("meshes", number(texts, written.meshes)),
        ("materials", number(texts, written.materials)),
        ("vertices", number(texts, written.vertices)),
        ("triangles", number(texts, written.triangles)),
        ("size", format::size(written.bytes, texts.language())),
    ]
    .map(|(name, value)| (name, FluentValue::from(value)));
    let mut notes = vec![
        file_note(texts, summary),
        (texts.format(T::SummaryCounts, &counts), palette::GRAY_1),
    ];
    notes.extend(interior_note(texts, summary));
    if written.textures > 0 || !written.texture_fallbacks.is_empty() {
        let args = [
            (
                "images",
                u64::try_from(written.textures).unwrap_or(u64::MAX).into(),
            ),
            ("textured", number(texts, written.textured_materials).into()),
            ("materials", number(texts, written.materials).into()),
        ];
        notes.push((texts.format(T::SummaryImages, &args), palette::GRAY_1));
    }
    warnings(
        texts,
        &mut notes,
        [
            (T::SummaryFallbacks, written.lod_fallbacks),
            (T::SummaryHidden, written.hidden_primitives),
            (T::SummaryMissingLayers, summary.missing_layers()),
        ],
    );
    if summary.issues().is_empty() {
        notes.push((texts.get(T::ReportAll), palette::GRAY_1));
    }
    notes
}

/// Adds a warning for each of `counts` that is not 0.
fn warnings<const N: usize>(
    texts: &Texts,
    notes: &mut Vec<(String, Color32)>,
    counts: [(T, usize); N],
) {
    for (text, value) in counts {
        if value > 0 {
            let args = [
                ("count", u64::try_from(value).unwrap_or(u64::MAX).into()),
                ("number", number(texts, value).into()),
            ];
            notes.push((texts.format(text, &args), palette::WARNING));
        }
    }
}

fn file_note(texts: &Texts, summary: &AssemblyExportSummary) -> (String, Color32) {
    let path = summary.file().display().to_string();
    (
        texts.format(T::SummaryFile, &[("path", path.into())]),
        palette::GRAY_1,
    )
}

fn interior_note(texts: &Texts, summary: &AssemblyExportSummary) -> Option<(String, Color32)> {
    let interior = summary.interior()?;
    let args = [
        ("containers", number(texts, interior.containers)),
        ("parts", number(texts, interior.parts)),
        ("models", number(texts, interior.model_parts)),
        ("modelled", number(texts, interior.modelled_parts)),
        ("lights", number(texts, interior.lights)),
    ]
    .map(|(name, value)| (name, FluentValue::from(value)));
    let text = if interior.of_entity {
        T::SummaryInterior
    } else {
        T::SummaryInteriorOpened
    };
    Some((texts.format(text, &args), palette::GRAY_1))
}

/// Returns the lines above the report of a Blender package, each with its colour.
fn package_notes(
    texts: &Texts,
    summary: &AssemblyExportSummary,
    package: &PackageSummary,
) -> Vec<(String, Color32)> {
    let counts = [
        ("parts", number(texts, summary.parts())),
        ("modelled", number(texts, summary.modelled_parts())),
        ("nodes", number(texts, package.nodes)),
        ("meshes", number(texts, package.meshes)),
        ("textures", number(texts, package.textures)),
        ("materials", number(texts, package.materials)),
        ("palettes", number(texts, package.palettes)),
        ("files", number(texts, package.files)),
        ("vertices", number(texts, package.vertices)),
        ("triangles", number(texts, package.triangles)),
        ("size", format::size(package.bytes, texts.language())),
    ]
    .map(|(name, value)| (name, FluentValue::from(value)));
    let mut notes = vec![
        file_note(texts, summary),
        (
            texts.format(T::SummaryPackageCounts, &counts),
            palette::GRAY_1,
        ),
    ];
    notes.extend(interior_note(texts, summary));
    warnings(
        texts,
        &mut notes,
        [
            (T::SummaryPackageTextures, package.texture_failures),
            (T::SummaryMissingLayers, summary.missing_layers()),
        ],
    );
    if summary.issues().is_empty() {
        notes.push((texts.get(T::ReportAll), palette::GRAY_1));
    }
    notes
}
