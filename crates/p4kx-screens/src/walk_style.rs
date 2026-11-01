// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{DefaultHasher, Hash, Hasher};
use std::rc::Rc;

use crate::canvas::Canvas;
use crate::style::{Modifiers, Subject, merge, resolve};
use crate::walk::Walk;
use crate::widget::Widget;

impl Walk<'_, '_, '_, '_> {
    pub(crate) fn push_chain(&mut self, key: &impl Hash) {
        let mut hasher = DefaultHasher::new();
        self.chain.last().copied().unwrap_or(0).hash(&mut hasher);
        key.hash(&mut hasher);
        self.chain.push(hasher.finish());
    }

    pub(crate) fn push_subject(&mut self, subject: &Subject, position: usize) {
        let place = self.position_key(position);
        let mut kids = DefaultHasher::new();
        for kid in &subject.kids {
            (&kid.kind, &kid.tags, kid.position).hash(&mut kids);
        }
        self.push_chain(&(&subject.kind, &subject.tags, place, kids.finish()));
    }

    /// Resolves the style of widget `index`: the entries, then the inner part of a text field or
    /// an icon, then the bound layout numbers under them, then the overrides of the instance, then
    /// the preset of a text field where nothing set its size.
    pub(crate) fn style_of(
        &mut self,
        canvas: &Canvas,
        (index, widget): (usize, &Widget),
        place: (usize, &[usize]),
    ) -> Rc<Modifiers> {
        let key = (
            canvas.guid,
            index,
            self.chain.last().copied().unwrap_or(0),
            self.override_key(),
            self.position_key(place.0),
        );
        if let Some(known) = self.resolved.get(&key) {
            return Rc::clone(known);
        }
        let subject = self.subject_of(canvas, (index, widget), place);
        let mut style = resolve(
            &self.sheets,
            widget,
            (&subject, &self.above),
            self.tags,
            &mut self.budget,
        );
        if let Some(part) = self.inner_part(widget, &subject) {
            merge(&mut style, &part);
        }
        self.under_bound_geometry(index, &mut style);
        self.apply_overrides(canvas, index, &mut style);
        self.apply_preset(widget, &mut style);
        let style = Rc::new(style);
        self.resolved.insert(key, Rc::clone(&style));
        style
    }

    /// Puts the layout numbers the bindings of the canvas set on widget `index` under those the
    /// entries set.
    fn under_bound_geometry(&self, index: usize, style: &mut Modifiers) {
        if let Some(bound) = self.rests.last().and_then(|state| state.geometry(index)) {
            let mut geometry = *bound;
            geometry.merge(&style.geometry);
            style.geometry = geometry;
        }
    }
}
