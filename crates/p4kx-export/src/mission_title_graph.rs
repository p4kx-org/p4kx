// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;
use std::collections::HashMap;

use p4kx_extraction::CancelToken;

use super::{AsciiKey, Localise, Resolve, Token, check_cancel, push};
use crate::CatalogError;

pub(super) const OPEN: &str = "~mission(";

#[derive(Clone, Copy)]
pub(super) struct Span {
    start: usize,
    end: usize,
}

impl Span {
    pub(super) fn read(self, text: &str) -> Result<&str, CatalogError> {
        text.get(self.start..self.end)
            .ok_or(CatalogError::MissionTextPlan)
    }
}

#[derive(Clone, Copy)]
pub(super) enum Segment<'source> {
    Literal(Span),
    Unresolved(Span),
    Runtime(&'source str),
    Follow { target: usize, name: Span },
}

pub(super) struct Node<'source> {
    key: &'source str,
    pub(super) text: Cow<'source, str>,
    pub(super) segments: Vec<Segment<'source>>,
}

pub(super) struct Graph<'source> {
    nodes: Vec<Node<'source>>,
    keys: HashMap<AsciiKey<'source>, Option<usize>>,
}

impl<'source> Graph<'source> {
    pub(super) fn build(
        key: &'source str,
        root: Cow<'source, str>,
        localise: &mut Localise<'_, 'source>,
        resolve: &mut Resolve<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, CatalogError> {
        let mut graph = Self {
            nodes: Vec::new(),
            keys: HashMap::new(),
        };
        let root = graph.add(key, root)?;
        graph.insert(key, Some(root))?;
        let mut next = 0;
        while next < graph.nodes.len() {
            check_cancel(cancel)?;
            let node = graph.node_mut(next)?;
            let key = node.key;
            let text = std::mem::take(&mut node.text);
            let segments = graph.segments(key, &text, localise, resolve, cancel)?;
            *graph.node_mut(next)? = Node {
                key,
                text,
                segments,
            };
            next += 1;
        }
        Ok(graph)
    }

    pub(super) fn node(&self, index: usize) -> Result<&Node<'source>, CatalogError> {
        self.nodes.get(index).ok_or(CatalogError::MissionTextPlan)
    }

    pub(super) const fn len(&self) -> usize {
        self.nodes.len()
    }

    fn node_mut(&mut self, index: usize) -> Result<&mut Node<'source>, CatalogError> {
        self.nodes
            .get_mut(index)
            .ok_or(CatalogError::MissionTextPlan)
    }

    fn add(&mut self, key: &'source str, text: Cow<'source, str>) -> Result<usize, CatalogError> {
        let index = self.nodes.len();
        push(
            &mut self.nodes,
            Node {
                key,
                text,
                segments: Vec::new(),
            },
        )?;
        Ok(index)
    }

    fn insert(&mut self, key: &'source str, target: Option<usize>) -> Result<(), CatalogError> {
        self.keys
            .try_reserve(1)
            .map_err(CatalogError::MissionTextAllocation)?;
        self.keys.insert(AsciiKey::locale(key), target);
        Ok(())
    }

    fn target(
        &mut self,
        key: &'source str,
        localise: &mut Localise<'_, 'source>,
    ) -> Result<Option<usize>, CatalogError> {
        if let Some(target) = self.keys.get(&AsciiKey::locale(key)) {
            return Ok(*target);
        }
        let target = localise(key)?.map(|text| self.add(key, text)).transpose()?;
        self.insert(key, target)?;
        Ok(target)
    }

    fn segments(
        &mut self,
        source_key: &'source str,
        text: &str,
        localise: &mut Localise<'_, 'source>,
        resolve: &mut Resolve<'_, 'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Vec<Segment<'source>>, CatalogError> {
        let mut segments = Vec::new();
        let mut rest = text;
        while let Some((before, suffix)) = rest.split_once(OPEN) {
            check_cancel(cancel)?;
            let Some((content, tail)) = suffix.split_once(')') else {
                break;
            };
            let start = text.len() - rest.len();
            literal(&mut segments, start, start + before.len())?;
            let token_start = text.len() - suffix.len();
            let (name, variant) = token(content);
            let span = Span {
                start: token_start,
                end: token_start + name.len(),
            };
            let segment = match resolve(source_key, name, variant)? {
                Some(Token::Placeholder(label)) => Segment::Runtime(label),
                Some(Token::Key(key)) => {
                    self.target(key, localise)?
                        .map_or(Segment::Unresolved(span), |target| Segment::Follow {
                            target,
                            name: span,
                        })
                }
                None => Segment::Unresolved(span),
            };
            push(&mut segments, segment)?;
            rest = tail;
        }
        literal(&mut segments, text.len() - rest.len(), text.len())?;
        Ok(segments)
    }
}

fn token(content: &str) -> (&str, Option<&str>) {
    content
        .split_once('|')
        .map_or((content, None), |(name, variant)| {
            (name, Some(variant).filter(|variant| !variant.is_empty()))
        })
}

fn literal(segments: &mut Vec<Segment<'_>>, start: usize, end: usize) -> Result<(), CatalogError> {
    if start < end {
        push(segments, Segment::Literal(Span { start, end }))?;
    }
    Ok(())
}
