// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::hash::{DefaultHasher, Hash, Hasher};

use p4kx_dataforge::Guid;

use crate::canvas::Canvas;
use crate::report::{ContentOmissionCode, OmissionInput};
use crate::style::{Modifiers, merge};
use crate::walk::Walk;
use crate::widget::{ARRAY_VARIABLE, CloneModifier, Widget};

const CLONE: &str = "BuildingBlocks_WidgetClone";
const LIST: &str = "BuildingBlocks_WidgetList";

const NO_TARGET: &str = "no target that is a widget of the canvas: nothing repeated";
const CYCLE: &str = "the template contains a clone of itself: not repeated";
const FROM_STATIC: &str = "list rows repeat the template without the values of their rows";

#[derive(Debug, Clone, Copy)]
enum KidsOmission {
    NoTarget,
    Cycle,
    RuntimeRows,
}

/// The modifiers of a clone, in force while the widget it instantiates is laid out.
#[derive(Debug)]
pub(crate) struct Override {
    canvas: Guid,
    root: usize,
    key: u64,
    items: Vec<CloneModifier>,
}

/// The children of a widget: those of its subtree, then the instances of its template.
#[derive(Debug, Default)]
pub(crate) struct KidsPlan {
    pub(crate) kids: Vec<usize>,
    pub(crate) own: usize,
    repeats: usize,
    note: Option<KidsOmission>,
    cut: bool,
}

impl Walk<'_, '_, '_, '_> {
    /// Returns the children of widget `index`: those whose parent it is, and for a clone its
    /// template once, for a list its template once per row of its static array.
    pub(crate) fn plan_kids(&self, canvas: &Canvas, index: usize, widget: &Widget) -> KidsPlan {
        let kids = canvas.children.get(index).cloned().unwrap_or_default();
        let mut plan = KidsPlan {
            own: kids.len(),
            kids,
            ..KidsPlan::default()
        };
        let is_list = widget.type_name == LIST;
        if !is_list && widget.type_name != CLONE {
            return plan;
        }
        let repeat = &widget.repeat;
        let Some(at) = repeat.target.and_then(|target| canvas.target_index(target)) else {
            plan.note = Some(KidsOmission::NoTarget);
            return plan;
        };
        if self.templates.contains(&(canvas.guid, at)) {
            plan.note = Some(KidsOmission::Cycle);
            return plan;
        }
        let rows = if is_list {
            let known = repeat
                .array_variable
                .as_deref()
                .and_then(|name| canvas.arrays.get(name));
            if let Some(&rows) = known {
                plan.cut = rows > self.limits.rows;
                rows.min(self.limits.rows)
            } else {
                plan.note = Some(KidsOmission::RuntimeRows);
                0
            }
        } else {
            1
        };
        plan.kids.extend(std::iter::repeat_n(at, rows));
        plan.repeats = rows;
        plan
    }

    /// Counts the instances `plan` makes against the limit and reports what it left out.
    pub(crate) fn commit_kids(&mut self, widget: &Widget, plan: &mut KidsPlan) {
        if let Some(omission) = plan.note {
            self.note_kids(widget, omission);
        }
        self.cx.report.truncated |= plan.cut;
        if plan.repeats == 0 {
            return;
        }
        if self.instances.saturating_add(plan.repeats) > self.limits.instances {
            self.cx.report.truncated = true;
            plan.kids.truncate(plan.own);
            plan.repeats = 0;
            return;
        }
        self.instances += plan.repeats;
        if widget.type_name == LIST {
            self.cx
                .report
                .note(&widget.type_name, FROM_STATIC, &widget.name);
        }
    }

    fn note_kids(&mut self, widget: &Widget, omission: KidsOmission) {
        let reason = match omission {
            KidsOmission::NoTarget => NO_TARGET,
            KidsOmission::Cycle => CYCLE,
            KidsOmission::RuntimeRows => {
                let source = widget.repeat.array_variable.as_deref();
                let (code, reason) = if source.is_some() {
                    (
                        ContentOmissionCode::RuntimeArray,
                        "list rows come from run-time data: none drawn at rest",
                    )
                } else {
                    (
                        ContentOmissionCode::SourceMissing,
                        "the Record field arrayVariable names no list rows",
                    )
                };
                self.cx.content_omission(
                    widget,
                    ARRAY_VARIABLE,
                    source,
                    code,
                    OmissionInput::WidgetField(ARRAY_VARIABLE),
                );
                reason
            }
        };
        self.cx.report.note(&widget.type_name, reason, &widget.name);
    }

    /// Returns what the modifiers in force add to the key of a resolved style or a measure.
    pub(crate) fn override_key(&self) -> u64 {
        self.overrides.last().map_or(0, |active| active.key)
    }

    /// Puts the modifiers of the clone `widget`, the widget `index` with the children `kids` of
    /// which `own` are its own, in force for its template; `true` when it did.
    pub(crate) fn push_override(
        &mut self,
        canvas: &Canvas,
        (index, widget): (usize, &Widget),
        (kids, own): (&[usize], usize),
    ) -> bool {
        let items = &widget.repeat.modifiers;
        let (false, true, Some(&root)) = (items.is_empty(), kids.len() > own, kids.last()) else {
            return false;
        };
        let (stack, per_clone) = (self.limits.clones_in_force, self.limits.modifiers_per_clone);
        if self.overrides.len() >= stack || items.len() > per_clone {
            self.cx.report.truncated = true;
        }
        if self.overrides.len() >= stack {
            return false;
        }
        let mut hasher = DefaultHasher::new();
        (self.override_key(), canvas.guid, index, root).hash(&mut hasher);
        self.overrides.push(Override {
            canvas: canvas.guid,
            root,
            key: hasher.finish(),
            items: items.iter().take(per_clone).cloned().collect(),
        });
        true
    }

    /// Lets the modifiers of the clones in force replace what the cascade made of widget `index`.
    pub(crate) fn apply_overrides(&self, canvas: &Canvas, index: usize, style: &mut Modifiers) {
        for active in &self.overrides {
            if active.canvas != canvas.guid {
                continue;
            }
            for item in &active.items {
                let hit = item.target.map_or_else(
                    || active.root == index,
                    |target| canvas.target_index(target) == Some(index),
                );
                if hit {
                    merge(style, &item.modifiers);
                }
            }
        }
    }
}
