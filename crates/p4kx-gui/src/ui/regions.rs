// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Context, Id, Rect, Ui};
use egui_extras::{TableBody, TableRow};

/// Where the main regions of the window stood in the last frame, for tests that check they do not
/// intersect.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Regions {
    /// The header with the menu. Dialogs shown after the panels of a frame also read it to stay
    /// below the header, so it is recorded before they are drawn.
    pub header: Option<Rect>,
    /// The status bar, read by dialogs like the header.
    pub status: Option<Rect>,
    /// The tab bar of the work surface.
    pub tabs: Option<Rect>,
    /// The rail that stays of a collapsed sidebar.
    pub rail: Option<Rect>,
    /// The sidebar.
    pub sidebar: Option<Rect>,
    /// The visible part of the work surface below the tab bar.
    pub work: Option<Rect>,
    /// The room the work surface lays its tab out in, whatever part of it is scrolled out.
    pub work_room: Option<Rect>,
    /// The `Ui` that holds what the work surface shows, whatever part of it is scrolled out.
    pub work_ui: Option<Id>,
    /// Content identifiers and visible rectangles of marked scroll areas inside the work surface.
    pub clips: Vec<(Id, Rect)>,
    /// The Entry list of the Browse tab, with its toolbar.
    pub list: Option<Rect>,
    /// The preview of the Browse tab, with everything its tab bar and body drew.
    pub preview: Option<Rect>,
    /// The modals and windows that stood above the window, the setup of an export and the
    /// windows that show its progress and outcome among them.
    pub dialogs: Vec<Rect>,
    /// Whether the body of the export setup holds more than its scroll area shows.
    pub scrolls: bool,
}

/// Returns the regions of the last frame drawn in `ctx`.
#[must_use]
pub fn regions(ctx: &Context) -> Regions {
    ctx.data(|data| data.get_temp::<Regions>(Id::NULL))
        .unwrap_or_default()
}

pub(super) fn reset(ctx: &Context) {
    ctx.data_mut(|data| data.insert_temp(Id::NULL, Regions::default()));
}

pub(super) fn record(ctx: &Context, set: impl FnOnce(&mut Regions)) {
    ctx.data_mut(|data| set(data.get_temp_mut_or_default::<Regions>(Id::NULL)));
}

/// Shows the `total` rows of `body` of `height` each like [`TableBody::rows`], after recording the
/// scroll content with [`clip`] for the layout tests.
pub(super) fn rows(
    mut body: TableBody<'_>,
    height: f32,
    total: usize,
    add_row: impl FnMut(TableRow<'_, '_>),
) {
    clip(body.ui_mut());
    body.rows(height, total, add_row);
}

/// Marks `ui`, the content of a scroll area, so that tests can tell what of it shows.
pub(super) fn clip(ui: &Ui) {
    let id = ui.unique_id();
    ui.ctx().accesskit_node_builder(id, |node| {
        node.set_role(egui::accesskit::Role::GenericContainer);
    });
    record(ui.ctx(), |regions| regions.clips.push((id, ui.clip_rect())));
}

#[cfg(test)]
mod tests {
    use egui::{CentralPanel, Context, RawInput};
    use egui_extras::{Column, TableBuilder};

    use super::{regions, reset, rows};

    #[test]
    fn the_rows_of_a_table_record_the_clip_of_its_scroll_area() {
        let ctx = Context::default();
        let mut output = ctx.run_ui(RawInput::default(), |ui| {
            reset(ui.ctx());
            CentralPanel::default().show(ui, |ui| {
                TableBuilder::new(ui)
                    .column(Column::remainder())
                    .body(|body| rows(body, 20.0, 3, |_row| {}));
            });
        });

        output.textures_delta.clear();
        let clips = regions(&ctx).clips;

        assert_eq!(clips.len(), 1, "one clip per table");
        assert!(clips[0].1.is_positive(), "the clip has a size");
    }
}
