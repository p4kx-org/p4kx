// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogFailureView, CatalogSummary, Kind, Playability};

use super::extraction::number;
use crate::{CatalogText as T, Texts, format};

const LISTED_DETAILS: usize = 8;

/// A warning of the game data that makes the catalog incomplete: its sentence, then the files it
/// names.
pub(super) struct Notice {
    pub(super) text: String,
    pub(super) details: Vec<String>,
}

/// Returns a notice for every warning of `summary`, each naming at most [`LISTED_DETAILS`] files.
pub(super) fn notices(texts: &Texts, summary: &CatalogSummary) -> Vec<Notice> {
    summary
        .warnings()
        .iter()
        .map(|warning| {
            let count = warning.count().unwrap_or_default();
            let text = texts.format(
                T::Warning,
                &[
                    ("kind", warning.kind().into()),
                    ("count", number(texts, count)),
                    ("message", warning.to_string().into()),
                ],
            );
            let mut details: Vec<String> = warning
                .details()
                .iter()
                .take(LISTED_DETAILS)
                .cloned()
                .collect();
            let more = warning.details().len().saturating_sub(LISTED_DETAILS);
            if more > 0 {
                details.push(texts.format(T::WarningMore, &[("number", number(texts, more))]));
            }
            Notice { text, details }
        })
        .collect()
}

/// Returns the line with the size of the catalog file against the upload limit, and the
/// uncompressed size beside it for a compressed catalog.
pub(super) fn sizes(texts: &Texts, summary: &CatalogSummary, limit: String) -> String {
    let size = format::decimal_size(summary.size(), texts.language());
    match summary.compressed_size() {
        Some(compressed) => texts.format(
            T::SummarySizeCompressed,
            &[
                (
                    "compressed",
                    format::decimal_size(compressed, texts.language()).into(),
                ),
                ("size", size.into()),
                ("limit", limit.into()),
            ],
        ),
        None => texts.format(
            T::SummarySize,
            &[("size", size.into()), ("limit", limit.into())],
        ),
    }
}

/// Formats one visible failure with the established catalogue text.
pub(super) fn failure_line(texts: &Texts, failure: CatalogFailureView<'_>) -> (String, usize) {
    let line = texts.format(
        T::SummaryFailure,
        &[
            ("path", failure.record_path().into()),
            ("guid", failure.guid().into()),
            ("reason", failure.reason().into()),
        ],
    );
    let reason = line.len().saturating_sub(failure.reason().len());
    (line, reason)
}
/// Returns the line of every kind, and below the line of the loadouts what they hold.
pub(super) fn kinds(texts: &Texts, summary: &CatalogSummary) -> Vec<String> {
    let mut lines = Vec::with_capacity(summary.kinds().len().saturating_add(1));
    for kind in summary.kinds() {
        lines.push(texts.format(
            T::SummaryKind,
            &[
                ("kind", kind.kind.name().into()),
                ("written", number(texts, kind.count)),
                ("skipped", number(texts, kind.skipped)),
                ("complete", if kind.complete { "yes" } else { "no" }.into()),
            ],
        ));
        if kind.kind == Kind::Loadouts {
            let loadouts = summary.loadouts();
            lines.push(texts.format(
                T::SummaryLoadouts,
                &[
                    ("ports", number(texts, loadouts.ports)),
                    ("unresolved", number(texts, loadouts.unresolved)),
                    ("docked", number(texts, loadouts.docked_vehicles)),
                    ("outside", number(texts, loadouts.outside)),
                ],
            ));
        }
    }
    lines
}

/// Returns the playability line of the items and of the ships.
pub(super) fn playability(texts: &Texts, summary: &CatalogSummary) -> Vec<String> {
    let counts = summary.playability();
    [("items", counts.items), ("ships", counts.ships)]
        .into_iter()
        .map(|(kind, counts)| {
            texts.format(
                T::SummaryPlayability,
                &[
                    ("kind", kind.into()),
                    ("player", number(texts, counts.of(Playability::Player))),
                    ("ai", number(texts, counts.of(Playability::Ai))),
                    ("mission", number(texts, counts.of(Playability::Mission))),
                    ("special", number(texts, counts.of(Playability::Special))),
                    ("unclassified", number(texts, counts.unclassified())),
                ],
            )
        })
        .collect()
}

pub(super) fn languages(texts: &Texts, summary: &CatalogSummary) -> Vec<String> {
    summary
        .texts()
        .iter()
        .map(|text| {
            let language = ("language", text.language.code().into());
            let missing = ("missing", number(texts, text.unresolved));
            if text.available {
                texts.format(
                    T::SummaryTexts,
                    &[
                        language,
                        ("resolved", number(texts, text.resolved)),
                        missing,
                        ("placeholders", number(texts, text.placeholder)),
                    ],
                )
            } else {
                texts.format(T::SummaryTextsUnavailable, &[language, missing])
            }
        })
        .collect()
}
