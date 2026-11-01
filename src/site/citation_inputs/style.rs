//! Inspect the citation layout and only the macros it can reach.

use std::collections::{BTreeMap, BTreeSet};

use hayagriva::citationberg::{
    IndependentStyle, LayoutRenderingElement, NamesChild, SortKey, TestPosition, TextTarget,
    taxonomy::{NumberVariable, Variable},
};
use thiserror::Error;

#[derive(Debug, Clone, Error)]
pub(in crate::site) enum Error {
    #[error("duplicate CSL macro {0:?}")]
    DuplicateMacro(String),
    #[error("{context} references unknown CSL macro {name:?}")]
    UnknownMacro { context: &'static str, name: String },
    #[error("CSL macros used by citations or the bibliography contain a cycle")]
    Cycle,
    #[error("the selected CSL style uses an unsupported position condition: {0}")]
    Position(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct StyleFacts {
    pub(super) strips_locator_periods: bool,
}

#[derive(Default)]
struct Scope<'a> {
    macros: BTreeSet<&'a str>,
    strips_periods: bool,
    has_locator: bool,
    stripping_macros: BTreeSet<&'a str>,
}

fn inspect<'a>(elements: &'a [LayoutRenderingElement], citation: bool) -> Result<Scope<'a>, Error> {
    let mut scope = Scope::default();
    let mut pending = vec![elements];
    while let Some(elements) = pending.pop() {
        for element in elements {
            match element {
                LayoutRenderingElement::Text(text) => match &text.target {
                    TextTarget::Macro { name } => {
                        scope.macros.insert(name);
                        if text.strip_periods {
                            scope.stripping_macros.insert(name);
                        }
                    }
                    TextTarget::Variable {
                        var: Variable::Number(NumberVariable::Locator),
                        ..
                    } => {
                        scope.has_locator = true;
                        scope.strips_periods |= text.strip_periods;
                    }
                    _ => {}
                },
                LayoutRenderingElement::Names(names) => {
                    for child in &names.children {
                        if let NamesChild::Substitute(substitute) = child {
                            pending.push(&substitute.children);
                        }
                    }
                }
                LayoutRenderingElement::Group(group) => pending.push(&group.children),
                LayoutRenderingElement::Choose(choose) => {
                    for branch in choose.branches() {
                        if citation && let Some(positions) = &branch.position {
                            for position in positions {
                                let unsupported = match position {
                                    TestPosition::Ibid => Some("ibid"),
                                    TestPosition::IbidWithLocator => Some("ibid-with-locator"),
                                    TestPosition::NearNote => Some("near-note"),
                                    _ => None,
                                };
                                if let Some(condition) = unsupported {
                                    return Err(Error::Position(condition));
                                }
                            }
                        }
                        pending.push(&branch.children);
                    }
                    if let Some(otherwise) = &choose.otherwise {
                        pending.push(&otherwise.children);
                    }
                }
                LayoutRenderingElement::Number(number) => {
                    scope.has_locator |= Variable::from(number.variable)
                        == Variable::Number(NumberVariable::Locator);
                }
                LayoutRenderingElement::Date(_) | LayoutRenderingElement::Label(_) => {}
            }
        }
    }
    Ok(scope)
}

/// Conservative for reachable period stripping because Hayagriva can carry
/// that state between renderings. Unused macros and bibliography-only rules do
/// not affect the citation sequence and are excluded.
pub(super) fn check(style: &IndependentStyle) -> Result<StyleFacts, Error> {
    let mut macros = BTreeMap::new();
    for item in &style.macros {
        if macros
            .insert(item.name.as_str(), item.children.as_slice())
            .is_some()
        {
            return Err(Error::DuplicateMacro(item.name.clone()));
        }
    }
    let mut citation = inspect(&style.citation.layout.elements, true)?;
    if let Some(sort) = &style.citation.sort {
        for key in &sort.keys {
            if let SortKey::MacroName { name, .. } = key {
                citation.macros.insert(name);
            }
        }
    }
    let mut strips_periods = citation.strips_periods;
    let mut stripping_macros = citation.stripping_macros;
    let mut pending: Vec<_> = citation.macros.into_iter().collect();
    let mut direct_locator = BTreeMap::<&str, bool>::new();
    let mut graph = BTreeMap::<&str, BTreeSet<&str>>::new();
    while let Some(name) = pending.pop() {
        if graph.contains_key(name) {
            continue;
        }
        let children = macros.get(name).ok_or_else(|| Error::UnknownMacro {
            context: "citation",
            name: name.to_owned(),
        })?;
        let scope = inspect(children, true)?;
        strips_periods |= scope.strips_periods;
        stripping_macros.extend(scope.stripping_macros);
        direct_locator.insert(name, scope.has_locator);
        pending.extend(scope.macros.iter().copied());
        graph.insert(name, scope.macros);
    }
    // Bibliography macros also execute during formatting. They need the same
    // recursion check, but their position and period rules do not describe
    // citation rendering.
    if let Some(bibliography) = &style.bibliography {
        let mut bibliography_scope = inspect(&bibliography.layout.elements, false)?;
        if let Some(sort) = &bibliography.sort {
            for key in &sort.keys {
                if let SortKey::MacroName { name, .. } = key {
                    bibliography_scope.macros.insert(name);
                }
            }
        }
        let mut pending: Vec<_> = bibliography_scope.macros.into_iter().collect();
        while let Some(name) = pending.pop() {
            if graph.contains_key(name) {
                continue;
            }
            let children = macros.get(name).ok_or_else(|| Error::UnknownMacro {
                context: "bibliography",
                name: name.to_owned(),
            })?;
            let scope = inspect(children, false)?;
            direct_locator.insert(name, scope.has_locator);
            pending.extend(scope.macros.iter().copied());
            graph.insert(name, scope.macros);
        }
    }
    // A recursive macro cannot be rendered safely. Check the reachable graph
    // iteratively so authored styles cannot exhaust the stack here.
    let mut incoming: BTreeMap<_, usize> = graph.keys().map(|name| (*name, 0)).collect();
    for references in graph.values() {
        for name in references {
            *incoming.get_mut(name).ok_or_else(|| Error::UnknownMacro {
                context: "citation",
                name: (*name).to_owned(),
            })? += 1;
        }
    }
    let mut ready: Vec<_> = incoming
        .iter()
        .filter_map(|(&name, &count)| (count == 0).then_some(name))
        .collect();
    let mut order = Vec::with_capacity(graph.len());
    while let Some(name) = ready.pop() {
        order.push(name);
        for target in &graph[name] {
            let count = incoming
                .get_mut(target)
                .ok_or_else(|| Error::UnknownMacro {
                    context: "citation",
                    name: (*target).to_owned(),
                })?;
            *count -= 1;
            if *count == 0 {
                ready.push(target);
            }
        }
    }
    if order.len() != graph.len() {
        return Err(Error::Cycle);
    }
    // A stripping macro wrapper can affect a nested numeric locator even
    // when a direct text locator resets that state. Treat either path as
    // risky; unrelated title and label formatting does not affect it.
    for name in order.into_iter().rev() {
        let nested_locator = graph[name]
            .iter()
            .any(|target| direct_locator.get(target).copied().unwrap_or(false));
        let has_locator = direct_locator.get(name).copied().unwrap_or(false) || nested_locator;
        direct_locator.insert(name, has_locator);
        if has_locator && stripping_macros.contains(name) {
            strips_periods = true;
        }
    }
    Ok(StyleFacts {
        strips_locator_periods: strips_periods,
    })
}
