// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Cursor;

use p4kx_extraction::SectionId;
use p4kx_extraction::bank::{BankHircSemantic, BankHircState, BankSource, BankStateRead};

use super::TestResult;
use super::node_groups::{self, Common, Coverage};

pub(super) fn scan(input: &[u8], visitor: &mut node_groups::Visitor<'_>) -> TestResult {
    let mut source = BankSource::open(Cursor::new(input), u64::try_from(input.len())?)?;
    let mut sections = source.section_cursor();
    let mut header = None;
    while let Some(section) = source.next_section(&mut sections) {
        let section = section?;
        match section.id() {
            SectionId::BKHD if header.is_none() => {
                header = Some(source.read_observed_header_for(&section)?);
            }
            SectionId::HIRC => {
                let header = header.as_ref().ok_or("original BKHD before HIRC")?;
                let mut objects = source.hirc(&section)?;
                while let Some(object) = objects.next_semantic_with_source_fields(header) {
                    visit(object?, header.header().version, visitor)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn visit(
    semantic: BankHircSemantic<'_, Cursor<&[u8]>>,
    version: u32,
    visitor: &mut node_groups::Visitor<'_>,
) -> TestResult {
    let object = *semantic.object();
    let owner = [object.section_ordinal(), object.ordinal()];
    match semantic {
        BankHircSemantic::State(state) => standalone(state, owner, version, visitor)?,
        BankHircSemantic::ActorMixer(mut actor) if version == 154 => {
            let fields = actor.fields();
            visitor.mark(Coverage::ActorMixer);
            visitor.common(
                owner,
                Common {
                    parent: &fields.node,
                    properties: &fields.properties,
                    aux: &fields.aux,
                    playback: &fields.playback,
                },
            )?;
            node_groups::actor(visitor, owner, &mut actor)?;
        }
        BankHircSemantic::ContainerWithNode(mut container) if version == 154 => {
            let fields = container.fields();
            visitor.mark(Coverage::Container);
            visitor.common(
                owner,
                Common {
                    parent: &fields.node,
                    properties: &fields.parameters.properties,
                    aux: &fields.parameters.aux,
                    playback: &fields.parameters.playback,
                },
            )?;
            node_groups::container(visitor, owner, &mut container)?;
        }
        BankHircSemantic::SoundWithNode(mut sound) => {
            let base = sound
                .sound()
                .fields
                .node
                .as_ref()
                .and_then(|node| node.base.as_ref());
            if let Some(base) = base {
                visitor.mark(Coverage::Sound);
                visitor.common(
                    owner,
                    Common {
                        parent: &base.fields,
                        properties: &base.properties,
                        aux: &base.aux,
                        playback: &base.playback,
                    },
                )?;
                node_groups::sound(visitor, owner, &mut sound)?;
            }
        }
        semantic if version == 154 => music(semantic, owner, visitor)?,
        _ => {}
    }
    Ok(())
}

fn music(
    semantic: BankHircSemantic<'_, Cursor<&[u8]>>,
    owner: [u64; 2],
    visitor: &mut node_groups::Visitor<'_>,
) -> TestResult {
    match semantic {
        BankHircSemantic::MusicSegment(mut node) => {
            node_groups::music(visitor, owner, &mut node.shared_node())?;
        }
        BankHircSemantic::MusicPlaylist(mut node) => {
            node_groups::music(visitor, owner, &mut node.shared_node())?;
        }
        BankHircSemantic::MusicSwitch(mut node) => {
            node_groups::music(visitor, owner, &mut node.shared_node())?;
        }
        BankHircSemantic::MusicTrack(mut node) => {
            node_groups::music(visitor, owner, &mut node.shared_node())?;
        }
        _ => {}
    }
    Ok(())
}

fn standalone(
    state: BankHircState,
    owner: [u64; 2],
    version: u32,
    visitor: &mut node_groups::Visitor<'_>,
) -> TestResult {
    visitor.mark(Coverage::StandaloneState);
    match state.status {
        BankStateRead::Decoded(state) => {
            visitor.mark(Coverage::TypedStandaloneState);
            node_groups::standalone(visitor, owner, version, Some(&state))
        }
        BankStateRead::Unsupported { .. } => node_groups::standalone(visitor, owner, version, None),
        _ => Ok(()),
    }
}
