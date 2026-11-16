// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::Trace;
use crate::context::Context;

#[derive(Debug)]
pub(super) enum TextPaintFailure {
    Empty,
    Unprepared,
    MissingColour,
    UnresolvedColour,
    Transparent,
    NoGlyphs,
}

impl TextPaintFailure {
    pub(super) const fn description(&self) -> &'static str {
        match self {
            Self::Empty => "the General label has empty or runtime-only effective text",
            Self::Unprepared => "the General label could not be prepared for drawing",
            Self::MissingColour => "the General label has no effective text colour",
            Self::UnresolvedColour => "the General label has an unresolved text colour",
            Self::Transparent => "the General label is fully transparent at rest",
            Self::NoGlyphs => "the prepared General label submitted no drawable glyphs",
        }
    }
}

impl Context<'_> {
    pub(crate) fn note_text_inputs(
        &mut self,
        prepared: bool,
        has_colour: bool,
        colour_alpha: Option<u8>,
        notes: &[String],
    ) {
        if let Some(trace) = &mut self.power_trace {
            trace.note_text_inputs(prepared, has_colour, colour_alpha, notes);
        }
    }

    pub(crate) fn note_text_without_glyphs(&mut self) {
        if let Some(trace) = &mut self.power_trace {
            trace.text_failure = Some(TextPaintFailure::NoGlyphs);
        }
    }
}

impl Trace {
    pub(crate) fn note_text_inputs(
        &mut self,
        prepared: bool,
        has_colour: bool,
        colour_alpha: Option<u8>,
        notes: &[String],
    ) {
        self.text_failure = if prepared {
            match (has_colour, colour_alpha) {
                (false, _) => Some(TextPaintFailure::MissingColour),
                (true, None) => Some(TextPaintFailure::UnresolvedColour),
                (true, Some(0)) => Some(TextPaintFailure::Transparent),
                (true, Some(_)) => None,
            }
        } else {
            let empty = notes
                .iter()
                .any(|note| note == "text is empty or filled in by the game at run time");
            Some(if empty {
                TextPaintFailure::Empty
            } else {
                TextPaintFailure::Unprepared
            })
        };
    }
}
