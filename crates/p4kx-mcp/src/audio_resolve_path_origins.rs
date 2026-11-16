// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{HircPathSource, WwiseEvent};
use p4kx_extraction::FieldOrigin;

use crate::forge_origins::ForgeOrigins;

use super::{Context, Evidence};

pub(super) struct Chain {
    origins: Vec<Step>,
    pub(super) route: FieldOrigin,
    pub(super) last_id: FieldOrigin,
}

struct Step {
    id: FieldOrigin,
    kind: FieldOrigin,
}

impl Chain {
    pub(super) fn read(
        context: &Context<'_>,
        event: &WwiseEvent,
        path_len: usize,
        sources: &[HircPathSource],
    ) -> Evidence<Self> {
        let start = context.event_selector(event)?;
        let mut inputs = vec![start.clone()];
        let mut origins = Vec::with_capacity(path_len);
        for ordinal in 0..path_len {
            let Some(original) = sources.get(ordinal) else {
                inputs.push(context.uses.origin(&[], false));
                origins.push(Step {
                    id: context.uses.origin(&[], false),
                    kind: context.uses.origin(&[], false),
                });
                continue;
            };
            let source = original.origins(context.archive, context.uses)?;
            let selected = if ordinal == 0 {
                context.union([&start, &source.inbound], true)?
            } else {
                source.inbound
            };
            let id = context.union([&selected, &source.id], true)?;
            let kind = context.union([&selected, &source.kind], true)?;
            inputs.push(id.clone());
            inputs.push(kind.clone());
            origins.push(Step { id, kind });
        }
        let route = context.union(&inputs, path_len == sources.len())?;
        let last_id = origins
            .last()
            .map_or_else(|| context.uses.origin(&[], false), |step| step.id.clone());
        Ok(Self {
            origins,
            route,
            last_id,
        })
    }

    pub(super) fn write(
        &self,
        context: &Context<'_>,
        output: &mut ForgeOrigins,
        address: &str,
    ) -> Evidence<()> {
        for (ordinal, step) in self.origins.iter().enumerate() {
            context.members(
                output,
                &format!("{address}/{ordinal}"),
                [("id", step.id.clone()), ("kind", step.kind.clone())],
            )?;
        }
        Ok(())
    }

    pub(super) fn object(
        &self,
        context: &Context<'_>,
        output: &mut ForgeOrigins,
        address: &str,
        row: &FieldOrigin,
    ) -> Evidence<()> {
        if let Some(step) = self.origins.last() {
            let id = context.union([row, &step.id], true)?;
            let kind = context.union([row, &step.kind], true)?;
            output.put(&format!("{address}/id"), id)?;
            output.put(&format!("{address}/kind"), kind)?;
        }
        Ok(())
    }
}
