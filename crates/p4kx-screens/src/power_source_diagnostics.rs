// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_dataforge::DataForge;

use super::{Check, DISPLAY, LABEL_BACKGROUND, Trace, VisitKind, black, heavy_font, parent_slot};
use crate::colour::ColorSpec;
use crate::report::Report;
use crate::widget::Widget;

#[derive(Debug, Default)]
pub(super) struct AuthoredWidgets {
    widgets: usize,
    text: usize,
    black_inline_text: usize,
    enabled_display_parents: usize,
    black_under_enabled_display: usize,
    black_under_signature_parent: usize,
    parents: ParentPredicates,
}

#[derive(Debug, Default)]
struct ParentPredicates {
    no_parent: usize,
    display: usize,
    enabled: usize,
    named_display: usize,
    signature_slot: usize,
    other_slot: usize,
    solid: usize,
    record: usize,
    other: usize,
    undecoded: usize,
}

impl ParentPredicates {
    fn observe(&mut self, parent: Option<&Widget>) {
        let Some(parent) = parent else {
            self.no_parent += 1;
            return;
        };
        self.display += usize::from(parent.type_name == DISPLAY);
        self.enabled += usize::from(parent.background_enabled);
        if parent.type_name != DISPLAY || !parent.background_enabled {
            return;
        }
        self.named_display += usize::from(!parent.name.is_empty());
        match &parent.background {
            Some(ColorSpec::Slot { name, .. }) if LABEL_BACKGROUND.contains(&name.as_str()) => {
                self.signature_slot += 1;
            }
            Some(ColorSpec::Slot { .. }) => self.other_slot += 1,
            Some(ColorSpec::Solid(_)) => self.solid += 1,
            Some(ColorSpec::Record { .. }) => self.record += 1,
            Some(ColorSpec::Other(_)) => self.other += 1,
            None => self.undecoded += 1,
        }
    }

    fn add(&mut self, other: &Self) {
        self.no_parent += other.no_parent;
        self.display += other.display;
        self.enabled += other.enabled;
        self.named_display += other.named_display;
        self.signature_slot += other.signature_slot;
        self.other_slot += other.other_slot;
        self.solid += other.solid;
        self.record += other.record;
        self.other += other.other;
        self.undecoded += other.undecoded;
    }

    fn write(&self, out: &mut impl Write, state: &str) -> Check {
        writeln!(
            out,
            "{state} authored black-inline text parents: {} absent, {} exact display type, {} background enabled; among enabled exact displays: {} named, {} historical slots, {} other slots, {} solid, {} colour-record, {} other form, {} missing or undecoded background",
            self.no_parent,
            self.display,
            self.enabled,
            self.named_display,
            self.signature_slot,
            self.other_slot,
            self.solid,
            self.record,
            self.other,
            self.undecoded,
        )
        .map_err(|_| "cannot report original parent predicates")
    }
}

impl AuthoredWidgets {
    pub(super) fn observe(&mut self, widget: &Widget, parent: Option<&Widget>) {
        self.widgets += 1;
        let display =
            parent.is_some_and(|widget| widget.type_name == DISPLAY && widget.background_enabled);
        self.enabled_display_parents += usize::from(display);
        if widget.text.is_none() {
            return;
        }
        self.text += 1;
        if !widget
            .inline
            .iter()
            .any(|entry| black(entry.modifiers.fill.as_ref()))
        {
            return;
        }
        self.black_inline_text += 1;
        self.black_under_enabled_display += usize::from(display);
        self.black_under_signature_parent += usize::from(parent.and_then(parent_slot).is_some());
        self.parents.observe(parent);
    }

    pub(super) fn add(&mut self, other: &Self) {
        self.widgets += other.widgets;
        self.text += other.text;
        self.black_inline_text += other.black_inline_text;
        self.enabled_display_parents += other.enabled_display_parents;
        self.black_under_enabled_display += other.black_under_enabled_display;
        self.black_under_signature_parent += other.black_under_signature_parent;
        self.parents.add(&other.parents);
    }

    pub(super) fn write(&self, out: &mut impl Write, state: &str) -> Check {
        writeln!(
            out,
            "{state} authored: {} widgets, {} text widgets, {} children of enabled display parents, {} text widgets with black inline fill ({} under enabled displays, {} under signature parents)",
            self.widgets,
            self.text,
            self.enabled_display_parents,
            self.black_inline_text,
            self.black_under_enabled_display,
            self.black_under_signature_parent,
        )
        .map_err(|_| "cannot report original widget classifications")?;
        self.parents.write(out, state)
    }
}

#[derive(Default)]
struct PaintedWidgets {
    scoped: usize,
    parent_signature: usize,
    inline_label: usize,
    effective_black: usize,
    font_present: usize,
    paired: usize,
    heavy: usize,
    paired_heavy: usize,
    unreadable_font: usize,
}

pub(super) fn rendered(
    out: &mut impl Write,
    state: &str,
    forge: &DataForge<'_>,
    trace: &Trace,
    report: &Report,
) -> Check {
    let mut seen = PaintedWidgets::default();
    for visit in trace.visits.iter().filter(|visit| visit.in_scope) {
        seen.scoped += 1;
        if matches!(visit.kind, VisitKind::Parent { .. }) {
            seen.parent_signature += 1;
        }
        let VisitKind::Label { font, black } = visit.kind else {
            continue;
        };
        seen.inline_label += 1;
        if !black {
            continue;
        }
        seen.effective_black += 1;
        let Some(font) = font else {
            continue;
        };
        seen.font_present += 1;
        let parent = visit.parent.and_then(|index| trace.visits.get(index));
        let paired = parent.is_some_and(|visit| matches!(visit.kind, VisitKind::Parent { .. }));
        seen.paired += usize::from(paired);
        match heavy_font(forge, font) {
            Ok(true) => {
                seen.heavy += 1;
                seen.paired_heavy += usize::from(paired);
            }
            Ok(false) => {}
            Err(_) => seen.unreadable_font += 1,
        }
    }
    let used = report.variables.iter().filter(|var| var.used).count();
    writeln!(
        out,
        "{state} rendered: {} scoped widget visits of {}, {} signature parents, {} original black-inline text visits, {} effective black, {} with a font ({} heavy, {} unreadable), {} paired with a signature parent ({} heavy), {used}/{} supplied variables recognized",
        seen.scoped,
        trace.visits.len(),
        seen.parent_signature,
        seen.inline_label,
        seen.effective_black,
        seen.font_present,
        seen.heavy,
        seen.unreadable_font,
        seen.paired,
        seen.paired_heavy,
        report.variables.len(),
    )
    .map_err(|_| "cannot report rendered widget classifications")
}
