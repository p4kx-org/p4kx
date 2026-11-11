// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::sync::Arc;

use egui::text::{CCursor, CCursorRange, CharIndex, LayoutJob, TextFormat};
use egui::text_edit::{TextEditOutput, TextEditState};
use egui::{
    Event, EventFilter, Frame, Galley, Id, InputState, Key, LayerId, Margin, Modifiers, PopupKind,
    Rect, Response, ScrollArea, Stroke, SurrenderFocusOn, TextBuffer, TextEdit, Ui, vec2,
};

use crate::query::CompletionState;
use crate::theme::desktop::{
    ALERT_DANGER_FILL, ERROR_UNDERLINE, QUERY_EDITOR_MAX_ROWS, QUERY_EDITOR_ROWS,
    TEXTAREA_LINE_HEIGHT, TEXTAREA_PADDING_X, TEXTAREA_PADDING_Y,
};
use crate::theme::palette;
use crate::theme::tokens::{FS_BODY, Weight, font};
use crate::ui::shortcuts::RUN_QUERY;
use crate::widgets::field;
use crate::widgets::frames::px;
use crate::widgets::text::plain;

/// What the user did in the editor this frame.
#[derive(Debug, Clone)]
pub(super) struct Edited {
    /// The text changed by typing.
    pub(super) changed: bool,
    /// The run shortcut asked to run the Query.
    pub(super) submitted: bool,
    /// Ctrl+Space asked for the names at the cursor.
    pub(super) asked: bool,
    /// The byte offset of the cursor while the editor has focus.
    pub(super) cursor: Option<usize>,
    /// Where the cursor stands on screen, the anchor of the completion list.
    pub(super) caret: Option<Rect>,
    /// The editor's response.
    pub(super) response: Response,
}

/// The last laid-out text, kept until the text, the mark, the width or the scale change.
#[derive(Clone, Default)]
struct Laid {
    text: String,
    mark: Option<Range<usize>>,
    width: u32,
    pixels_per_point: u32,
    galley: Option<Arc<Galley>>,
}

/// The id of the editor.
pub(super) fn editor_id() -> Id {
    Id::unique("p4kx-query-editor")
}

/// The id of the completion list of the editor.
pub(super) fn popup_id() -> Id {
    editor_id().with("completion")
}

/// Shows the Query editor, a `textarea` labelled by `label`, with the bytes `mark` of the text
/// underlined as an error; the run shortcut submits the Query. While the
/// completion list is open, the arrow keys move in it, Enter and Tab accept and Escape closes
/// it.
pub(super) fn show(
    ui: &mut Ui,
    (text, completion): (&mut String, &mut CompletionState),
    (hint, label): (&str, Id),
    mark: Option<&Range<usize>>,
) -> Edited {
    let open = completion.shown().is_some();
    let (submitted, asked) = keys(ui, completion, text);
    let (on_popup, surrender) = hold_focus_on_popup(ui, open);
    let bloom = field::bloom_slot(ui);
    let open = completion.shown().is_some();
    let output = text_area(ui, text, hint, mark, open);
    ui.ctx()
        .options_mut(|options| options.input_options.surrender_focus_on = surrender);
    let response = output.response.response.clone().labelled_by(label);
    if response.has_focus() {
        field::paint_bloom(ui, bloom, response.rect);
    }
    let char_cursor = output
        .cursor_range
        .filter(|_| response.has_focus())
        .map(|range| range.primary);
    let caret = char_cursor.map(|cursor| {
        output
            .galley
            .pos_from_cursor(cursor)
            .translate(output.galley_pos.to_vec2())
    });
    let cursor = char_cursor.map(|cursor| byte_of(text, cursor.index.0));
    if !response.has_focus() && !on_popup {
        completion.close();
    }
    Edited {
        changed: response.changed(),
        submitted,
        asked,
        cursor,
        caret,
        response,
    }
}

/// Returns whether the run and completion shortcuts were pressed, consuming handled keys.
fn keys(ui: &Ui, completion: &mut CompletionState, text: &mut String) -> (bool, bool) {
    if !ui.memory(|memory| memory.has_focus(editor_id())) {
        return (false, false);
    }
    let open = completion.shown().is_some();
    let (mut submitted, mut asked, mut accepted) = (false, false, None);
    ui.input_mut(|input| {
        submitted = input.consume_shortcut(&RUN_QUERY);
        asked = input.consume_key(Modifiers::COMMAND, Key::Space);
        if open {
            accepted = popup_keys(input, completion, text);
        }
    });
    if let Some(end) = accepted {
        place_cursor(ui.ctx(), text, end);
    }
    (submitted, asked)
}

/// Keeps the editor focused while the pointer is on the open completion list, so a click on
/// it lands; returns whether it is there and the setting to restore after the editor.
fn hold_focus_on_popup(ui: &Ui, open: bool) -> (bool, SurrenderFocusOn) {
    let popup_layer = LayerId::new(PopupKind::Popup.order(), popup_id());
    let on_popup = open
        && ui
            .ctx()
            .pointer_interact_pos()
            .and_then(|pos| ui.ctx().layer_id_at(pos))
            == Some(popup_layer);
    let surrender = ui
        .ctx()
        .options(|options| options.input_options.surrender_focus_on);
    if on_popup {
        ui.ctx().options_mut(|options| {
            options.input_options.surrender_focus_on = SurrenderFocusOn::Never;
        });
    }
    (on_popup, surrender)
}

/// Shows the text area, keeping Tab and Escape while the completion list is `open`.
fn text_area(
    ui: &mut Ui,
    text: &mut String,
    hint: &str,
    mark: Option<&Range<usize>>,
    open: bool,
) -> TextEditOutput {
    let rows = |rows: usize| {
        let line_height = FS_BODY * TEXTAREA_LINE_HEIGHT;
        line_height.mul_add(
            f32::from(u8::try_from(rows).unwrap_or(u8::MAX)),
            2.0 * TEXTAREA_PADDING_Y,
        )
    };
    let height = rows(QUERY_EDITOR_ROWS);
    let mut layouter =
        |ui: &Ui, buffer: &dyn TextBuffer, width: f32| laid_out(ui, buffer.as_str(), mark, width);
    let edit = TextEdit::multiline(text)
        .id(editor_id())
        .hint_text(plain(hint, FS_BODY, Weight::Regular).italics())
        .font(font(FS_BODY, Weight::Regular))
        .frame(Frame::NONE.inner_margin(Margin::symmetric(
            px(TEXTAREA_PADDING_X),
            px(TEXTAREA_PADDING_Y),
        )))
        .min_size(vec2(0.0, height))
        .desired_width(f32::INFINITY)
        .event_filter(EventFilter {
            tab: open,
            escape: open,
            horizontal_arrows: true,
            vertical_arrows: true,
        })
        .layouter(&mut layouter);
    ScrollArea::vertical()
        .id_salt("p4kx-query-editor-scroll")
        .max_height(rows(QUERY_EDITOR_MAX_ROWS))
        .auto_shrink([false, true])
        .show(ui, |ui| {
            let frame = field::frame_slot(ui);
            let output = edit.show(ui);
            field::paint_frame(ui, frame, output.response.rect, output.response.has_focus());
            output
        })
        .inner
}

/// Handles the keys of the open completion list before the editor sees them; Enter passes
/// through and closes the list when the active name is written already.
/// Returns the byte offset behind an accepted name.
fn popup_keys(
    input: &mut InputState,
    completion: &mut CompletionState,
    text: &mut String,
) -> Option<usize> {
    let down = consume_plain(input, Key::ArrowDown);
    let up = consume_plain(input, Key::ArrowUp);
    let enter =
        if down == up && pressed_plain(input, Key::Enter) && completion.active_is_written(text) {
            completion.close();
            0
        } else {
            consume_plain(input, Key::Enter)
        };
    let accept = enter + consume_plain(input, Key::Tab) > 0;
    let dismiss = consume_plain(input, Key::Escape) > 0;
    let steps = isize::try_from(down).unwrap_or(0) - isize::try_from(up).unwrap_or(0);
    if steps != 0 {
        completion.step(steps);
    }
    if dismiss {
        completion.dismiss();
        None
    } else if accept {
        completion.accept(text)
    } else {
        None
    }
}

/// Puts the cursor of the editor at byte `end` of `text`.
pub(super) fn place_cursor(ctx: &egui::Context, text: &str, end: usize) {
    let id = editor_id();
    let index = text.get(..end).map_or(0, |before| before.chars().count());
    let mut state = TextEditState::load(ctx, id).unwrap_or_default();
    state
        .cursor
        .set_char_range(Some(CCursorRange::one(CCursor::new(CharIndex(index)))));
    state.store(ctx, id);
}

fn consume_plain(input: &mut InputState, key: Key) -> usize {
    let mut count = 0;
    input.events.retain(|event| {
        let hit = is_plain(event, key);
        count += usize::from(hit);
        !hit
    });
    count
}

fn pressed_plain(input: &InputState, key: Key) -> bool {
    input.events.iter().any(|event| is_plain(event, key))
}

fn is_plain(event: &Event, key: Key) -> bool {
    matches!(
        event,
        Event::Key {
            key: pressed,
            modifiers,
            pressed: true,
            ..
        } if *pressed == key && modifiers.is_none()
    )
}

fn byte_of(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte)
}

fn laid_out(ui: &Ui, text: &str, mark: Option<&Range<usize>>, width: f32) -> Arc<Galley> {
    let id = Id::unique("p4kx-query-editor-galley");
    let width_key = width.to_bits();
    let scale = ui.ctx().pixels_per_point().to_bits();
    let known = ui.data_mut(|data| {
        let laid = data.get_temp_mut_or_default::<Laid>(id);
        (laid.text == text
            && laid.mark.as_ref() == mark
            && laid.width == width_key
            && laid.pixels_per_point == scale)
            .then(|| laid.galley.clone())
            .flatten()
    });
    if let Some(galley) = known {
        return galley;
    }
    let mut job = LayoutJob::default();
    job.wrap.max_width = width;
    let normal = TextFormat {
        font_id: font(FS_BODY, Weight::Regular),
        color: palette::GRAY_1,
        line_height: Some(FS_BODY * TEXTAREA_LINE_HEIGHT),
        ..TextFormat::default()
    };
    let marked = TextFormat {
        background: ALERT_DANGER_FILL,
        underline: Stroke::new(ERROR_UNDERLINE, palette::DANGER),
        ..normal.clone()
    };
    let span = mark.and_then(|span| shown_span(text, span));
    match span {
        Some(span) => {
            job.append(
                text.get(..span.start).unwrap_or_default(),
                0.0,
                normal.clone(),
            );
            job.append(text.get(span.clone()).unwrap_or_default(), 0.0, marked);
            job.append(text.get(span.end..).unwrap_or_default(), 0.0, normal);
        }
        None => job.append(text, 0.0, normal),
    }
    let galley = ui.ctx().fonts_mut(|fonts| fonts.layout_job(job));
    ui.data_mut(|data| {
        let laid = data.get_temp_mut_or_default::<Laid>(id);
        text.clone_into(&mut laid.text);
        laid.mark = mark.cloned();
        laid.width = width_key;
        laid.pixels_per_point = scale;
        laid.galley = Some(Arc::clone(&galley));
    });
    galley
}

/// Returns the bytes of `text` to mark for `span`: the span itself, or the character at an empty
/// span; `None` when nothing of the text can be marked.
fn shown_span(text: &str, span: &Range<usize>) -> Option<Range<usize>> {
    if span.start > text.len() || !text.is_char_boundary(span.start) {
        return None;
    }
    let end = if span.is_empty() {
        span.start + text.get(span.start..)?.chars().next()?.len_utf8()
    } else {
        span.end.min(text.len())
    };
    text.is_char_boundary(end).then_some(span.start..end)
}

#[cfg(test)]
mod tests {
    use super::shown_span;

    #[test]
    fn a_span_is_marked_as_it_is() {
        assert_eq!(
            shown_span("from Item select mas", &(17..20)),
            Some(17..20),
            "span"
        );
    }

    #[test]
    fn an_empty_span_marks_the_character_at_it() {
        assert_eq!(shown_span("from Ä x", &(5..5)), Some(5..7), "two bytes");
    }

    #[test]
    fn an_empty_span_at_the_end_marks_nothing() {
        assert_eq!(shown_span("from", &(4..4)), None, "end");
    }

    #[test]
    fn a_span_of_another_text_marks_nothing() {
        assert_eq!(shown_span("from Ä", &(6..6)), None, "inside a character");
        assert_eq!(shown_span("from", &(9..12)), None, "beyond the end");
    }
}
