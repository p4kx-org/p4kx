// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::audio::{BreakReason, BrokenLink};
use p4kx_extraction::FieldOrigin;

use crate::forge_origins::ForgeOrigins;

use super::path::Chain;
use super::{Context, Evidence};

pub(super) fn write(
    context: &Context<'_>,
    output: &mut ForgeOrigins,
    links: &[&BrokenLink<'_>],
    first_page_total: Option<&FieldOrigin>,
) -> Evidence<()> {
    let mut breaks = Vec::new();
    if links.is_empty() {
        return output.put("breaks", context.union(first_page_total, true)?);
    }
    for (index, link) in links.iter().enumerate() {
        let address = format!("breaks/{index}");
        breaks.push(Facts::read(context, link)?.write(context, output, &address, link)?);
    }
    let listed = context.union(&breaks, true)?;
    output.put("breaks", listed)
}

pub(super) struct Facts {
    chain: Chain,
    pub(super) reason: FieldOrigin,
    target: FieldOrigin,
}

impl Facts {
    pub(super) fn read(context: &Context<'_>, link: &BrokenLink<'_>) -> Evidence<Self> {
        let chain = Chain::read(
            context,
            link.event(),
            link.path().len(),
            link.path_sources(),
        )?;
        let selected = link.failed_edge_origin(context.archive, context.uses)?;
        let target = match link.reason() {
            BreakReason::Missing { .. }
            | BreakReason::OtherSoundbank { .. }
            | BreakReason::Cycle { .. } => context.union([&chain.route, &selected], true)?,
            _ => chain.route.clone(),
        };
        let reason = match link.reason() {
            BreakReason::NoSoundbank => context.union(
                std::iter::once(&chain.route).chain(&context.no_bank),
                context.no_bank.is_some(),
            )?,
            BreakReason::Unreadable(_) => context.union([&chain.route, &selected], false)?,
            BreakReason::NoLocalisation { .. } => {
                context.union([&chain.route, &context.preferred, &context.fallback], false)?
            }
            BreakReason::UnknownLayout { .. } | BreakReason::Malformed(_) => {
                context.union([&chain.route], false)?
            }
            BreakReason::TooManyObjects { .. } => context.union([&chain.route], false)?,
            _ => target.clone(),
        };
        Ok(Self {
            chain,
            reason,
            target,
        })
    }

    fn write(
        self,
        context: &Context<'_>,
        output: &mut ForgeOrigins,
        address: &str,
        link: &BrokenLink<'_>,
    ) -> Evidence<FieldOrigin> {
        self.chain
            .write(context, output, &format!("{address}/path"))?;
        self.chain.object(
            context,
            output,
            &format!("{address}/object"),
            &self.chain.route,
        )?;
        let bank = if link.soundbank().is_some() {
            context.bank(
                output,
                &format!("{address}/soundbank"),
                link.entry(context.archive),
                &self.chain.route,
            )?
        } else {
            self.reason.clone()
        };
        let object = if link.object().is_some() {
            self.chain.route.clone()
        } else {
            self.reason.clone()
        };
        context.members(
            output,
            address,
            [
                ("event", context.event_name(link.event())?),
                ("soundbank", bank),
                ("object", object),
                ("target", self.target),
                ("path", self.chain.route),
                ("kind", self.reason.clone()),
                ("message", self.reason),
            ],
        )
    }
}
