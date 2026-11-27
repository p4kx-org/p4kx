// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// What a character occupies on its line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Item {
    pub(crate) kind: Kind,
    pub(crate) advance: f32,
    pub(crate) ascent: f32,
    pub(crate) descent: f32,
    pub(crate) align: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Glyph,
    Space,
    Newline,
}

/// The area lines are laid out in, in twips.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Area {
    pub(crate) left: f32,
    pub(crate) width: f32,
    pub(crate) top: f32,
    pub(crate) wrap: bool,
    pub(crate) indent: f32,
    pub(crate) leading: f32,
}

/// Where the item `item` has its pen and baseline.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Placed {
    pub(crate) item: usize,
    pub(crate) x: f32,
    pub(crate) baseline: f32,
    pub(crate) inspected: usize,
}

const ALIGN_RIGHT: u8 = 1;
const ALIGN_CENTRE: u8 = 2;

/// Breaks `items` into lines at newlines and, when the area wraps, after the last space that
/// fits, aligns them and appends the position of every glyph to `out`.
pub(crate) fn layout(items: &[Item], area: &Area, out: &mut Vec<Placed>) {
    let mut top = area.top;
    let mut start = 0;
    let mut first = true;
    let mut inspected = 0;
    loop {
        let boundary = line_end(items, start, area, first);
        let end = boundary.end;
        inspected = inspected.max(boundary.inspected);
        let line = items.get(start..end).unwrap_or_default();
        let next = if items
            .get(end)
            .is_some_and(|item| item.kind == Kind::Newline)
        {
            end + 1
        } else {
            end
        };
        let (ascent, descent) = metrics(line, items.get(end));
        let baseline = top + ascent;
        let indent = if first { area.indent } else { 0.0 };
        let width = indent + trimmed_width(line);
        let free = (area.width - width).max(0.0);
        let align = line
            .first()
            .or_else(|| items.get(end))
            .map_or(0, |item| item.align);
        let mut x = area.left
            + indent
            + match align {
                ALIGN_RIGHT => free,
                ALIGN_CENTRE => free / 2.0,
                _ => 0.0,
            };
        for (offset, item) in line.iter().enumerate() {
            if item.kind == Kind::Glyph {
                out.push(Placed {
                    item: start + offset,
                    x,
                    baseline,
                    inspected,
                });
            }
            x += item.advance;
        }
        top = baseline + descent + area.leading;
        first = false;
        if end >= items.len() {
            break;
        }
        start = next;
    }
}

struct LineEnd {
    end: usize,
    inspected: usize,
}

fn line_end(items: &[Item], start: usize, area: &Area, first: bool) -> LineEnd {
    let limit = area.width - if first { area.indent } else { 0.0 };
    let mut width = 0.0;
    let mut break_after = None;
    for (offset, item) in items.get(start..).unwrap_or_default().iter().enumerate() {
        let at = start + offset;
        if item.kind == Kind::Newline {
            return LineEnd {
                end: at,
                inspected: at + 1,
            };
        }
        if area.wrap && at > start && item.kind == Kind::Glyph && width + item.advance > limit {
            return LineEnd {
                end: break_after.unwrap_or(at),
                inspected: at + 1,
            };
        }
        width += item.advance;
        if item.kind == Kind::Space {
            break_after = Some(at + 1);
        }
    }
    LineEnd {
        end: items.len(),
        inspected: items.len(),
    }
}

fn trimmed_width(line: &[Item]) -> f32 {
    let trailing = line
        .iter()
        .rev()
        .take_while(|item| item.kind == Kind::Space)
        .count();
    line.get(..line.len() - trailing)
        .unwrap_or_default()
        .iter()
        .map(|item| item.advance)
        .sum()
}

fn metrics(line: &[Item], newline: Option<&Item>) -> (f32, f32) {
    let mut ascent = 0.0_f32;
    let mut descent = 0.0_f32;
    for item in line.iter().chain(newline.filter(|_| line.is_empty())) {
        ascent = ascent.max(item.ascent);
        descent = descent.max(item.descent);
    }
    (ascent, descent)
}

#[cfg(test)]
mod tests {
    use super::{Area, Item, Kind, Placed, layout};

    fn item(kind: Kind, advance: f32) -> Item {
        Item {
            kind,
            advance,
            ascent: 8.0,
            descent: 2.0,
            align: 0,
        }
    }

    fn area(width: f32, wrap: bool) -> Area {
        Area {
            left: 100.0,
            width,
            top: 0.0,
            wrap,
            indent: 0.0,
            leading: 0.0,
        }
    }

    fn run(items: &[Item], area: &Area) -> Vec<Placed> {
        let mut out = Vec::new();
        layout(items, area, &mut out);
        out
    }

    #[test]
    fn glyphs_advance_along_the_baseline() {
        let items = [item(Kind::Glyph, 5.0), item(Kind::Glyph, 6.0)];
        let placed = run(&items, &area(100.0, false));
        assert_eq!(
            placed.iter().map(|p| p.x).collect::<Vec<_>>(),
            [100.0, 105.0]
        );
        assert_eq!(
            placed.iter().map(|p| p.baseline).collect::<Vec<_>>(),
            [8.0, 8.0]
        );
    }

    #[test]
    fn newlines_start_lines_and_an_empty_line_keeps_its_height() {
        let items = [
            item(Kind::Glyph, 5.0),
            item(Kind::Newline, 0.0),
            item(Kind::Newline, 0.0),
            item(Kind::Glyph, 5.0),
        ];
        let placed = run(&items, &area(100.0, false));
        let baselines: Vec<f32> = placed.iter().map(|p| p.baseline).collect();
        assert_eq!(baselines, [8.0, 28.0]);
    }

    #[test]
    fn words_wrap_after_the_last_space_that_fits() {
        let items = [
            item(Kind::Glyph, 4.0),
            item(Kind::Space, 2.0),
            item(Kind::Glyph, 4.0),
            item(Kind::Glyph, 4.0),
        ];
        let placed = run(&items, &area(8.0, true));
        let rows: Vec<(usize, f32)> = placed.iter().map(|p| (p.item, p.baseline)).collect();
        assert_eq!(rows, [(0, 8.0), (2, 18.0), (3, 18.0)]);
        let long = [item(Kind::Glyph, 6.0), item(Kind::Glyph, 6.0)];
        assert_eq!(run(&long, &area(8.0, true)).len(), 2, "a long word breaks");
    }

    #[test]
    fn consulted_prefix_stops_at_the_newline_or_the_wrap_lookahead() {
        let glyph = item(Kind::Glyph, 4.0);
        let separate = [glyph, item(Kind::Newline, 0.0), glyph, glyph];
        let prefixes: Vec<_> = run(&separate, &area(50.0, false))
            .iter()
            .map(|placed| placed.inspected)
            .collect();
        assert_eq!(
            prefixes,
            [2, 4, 4],
            "later lines cannot change earlier positions"
        );

        let wrapped = [glyph, item(Kind::Space, 2.0), glyph, glyph];
        let prefixes: Vec<_> = run(&wrapped, &area(8.0, true))
            .iter()
            .map(|placed| placed.inspected)
            .collect();
        assert_eq!(
            prefixes,
            [3, 4, 4],
            "the first excluded glyph participates in wrapping"
        );
    }

    #[test]
    fn lines_align_right_and_centre_within_the_width() {
        let mut right = item(Kind::Glyph, 10.0);
        right.align = 1;
        assert_eq!(run(&[right], &area(50.0, false))[0].x, 140.0);
        let mut centre = item(Kind::Glyph, 10.0);
        centre.align = 2;
        assert_eq!(run(&[centre], &area(50.0, false))[0].x, 120.0);
    }

    #[test]
    fn no_items_and_a_trailing_newline_end() {
        assert_eq!(run(&[], &area(10.0, true)).len(), 0);
        let items = [item(Kind::Glyph, 1.0), item(Kind::Newline, 0.0)];
        assert_eq!(run(&items, &area(10.0, true)).len(), 1);
    }
}
