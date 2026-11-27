// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{BankNodeAuxFields, BankNodeUserAuxBuses};

use crate::audio_bank_node_declaration_payload::{
    AuxObservations, NodeDeclarationPayload, NodeReferenceStatus,
};
use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-node-aux";

pub(crate) fn summary(
    declarations: &Declarations<'_, '_>,
    disposition: Disposition<'_>,
    original: Option<&BankNodeAuxFields>,
    original_buses: Option<&BankNodeUserAuxBuses>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::Aux);
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(original) = original {
        observations(&sites, &mut prepared, original, original_buses)?
    } else {
        AuxObservations::default()
    };
    let (payload, status, issue) = declarations.summary(
        &sites,
        &mut prepared,
        disposition,
        &Coverage::read(
            original.and_then(|aux| aux.start),
            original.and_then(BankNodeAuxFields::complete_range),
        ),
        observed,
        "audio.bank.invalid-node-aux",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(NAMESPACE, sites.indices().to_vec()),
        status,
        &NodeDeclarationPayload::Aux(&payload),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankNodeAuxFields,
    buses: Option<&BankNodeUserAuxBuses>,
) -> Result<AuxObservations, MediaError> {
    let flags = sites.scalar(
        prepared,
        "observed/flags",
        original.flags.as_ref(),
        "nodeAuxFlags",
    )?;
    let reflections = original.reflections_bus_id.as_ref();
    let reflections_bus_id = sites.scalar(
        prepared,
        "observed/reflections_bus_id",
        reflections,
        "nodeReflectionsAuxBusId",
    )?;
    let user_bus_ids = buses
        .map(|buses| {
            let mut ids = [const { None }; 4];
            for (ordinal, (input, output)) in buses.iter().zip(&mut ids).enumerate() {
                *output = sites.scalar(
                    prepared,
                    &format!("observed/user_bus_ids/{ordinal}"),
                    input.as_ref(),
                    "nodeUserAuxBusId",
                )?;
            }
            Ok::<_, MediaError>(ids)
        })
        .transpose()?;
    let reference = reflections.or_else(|| buses.and_then(|ids| ids.iter().flatten().next()));
    let reference_resolution = reference
        .map(|id| {
            sites.converted(
                prepared,
                "observed/reference_resolution",
                &id.range,
                "nodeAuxReferenceUnchecked",
            )?;
            Ok::<_, MediaError>(NodeReferenceStatus::Unchecked)
        })
        .transpose()?;
    Ok(AuxObservations {
        flags,
        user_bus_ids,
        reflections_bus_id,
        reference_resolution,
    })
}
