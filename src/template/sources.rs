//! Template discovery and kit-aware template reference checks.
//!
//! Tera intentionally sees one completed template set. This module constructs
//! that set without allowing its ordinary global lookup rules to blur the
//! ownership boundary between root and kit templates.

#[path = "source_syntax.rs"]
mod source_syntax;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use miette::{NamedSource, SourceSpan};
use walkdir::WalkDir;

use self::source_syntax::{
    TemplateReference, component_calls, component_definitions, rewrite_kit_asset_calls,
    template_function_call, template_references,
};
use super::{Error, TemplateDiagnostic, TemplateName, WalkFailure};
use crate::{kit, source_diagnostic::DiagnosticPaths, source_path::LogicalPath};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Root,
    Kit,
}

/// Template sources paired with their ownership while reference policy is
/// checked. Ownership is intentionally discarded before Tera renders.
pub(super) struct Catalog {
    sources: BTreeMap<TemplateName, NamedSource<String>>,
    rewritten_sources: BTreeMap<TemplateName, String>,
    origins: BTreeMap<TemplateName, Origin>,
}

impl Catalog {
    pub(super) fn load(
        root_templates: &Path,
        selected_kit: Option<&kit::Selected>,
        diagnostics: &DiagnosticPaths,
    ) -> Result<Self, Error> {
        let mut discovered = discover(root_templates, Origin::Root, diagnostics)?;
        if let Some(template) = discovered
            .iter()
            .find(|template| is_kit_namespace(&template.name))
        {
            return Err(Error::ReservedKitNamespace {
                path: diagnostics.display_path(&template.path),
            });
        }

        if let Some(selected_kit) = selected_kit {
            let kit_templates = selected_kit.paths().templates();
            let kit_name = selected_kit.manifest().name();
            let kit_templates = discover(kit_templates, Origin::Kit, diagnostics)?
                .into_iter()
                .map(|template| {
                    let name =
                        kit_template_name(kit_name, &template.name, &template.path, diagnostics)?;
                    Ok(DiscoveredTemplate {
                        path: template.path,
                        name,
                        origin: template.origin,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?;
            validate_public_templates(selected_kit, &kit_templates, diagnostics)?;
            discovered.extend(kit_templates);
        }

        if let Some(collision) = crate::source_path::normalization_collision(
            discovered
                .iter()
                .map(|template| (template.name.0.as_path().into(), template.path.clone())),
        ) {
            return Err(Error::NormalizationCollision(Box::new(collision)));
        }

        let mut sources = BTreeMap::new();
        let mut rewritten_sources = BTreeMap::new();
        let mut origins = BTreeMap::new();
        for template in discovered {
            let source = fs::read_to_string(&template.path).map_err(|source| Error::Read {
                path: diagnostics.display_path(&template.path),
                source,
            })?;
            let rewritten = (template.origin == Origin::Kit)
                .then(|| rewrite_kit_asset_calls(&source))
                .flatten();
            let named = diagnostics.named_source(&template.path, source, "Tera");
            origins.insert(template.name.clone(), template.origin);
            if let Some(rewritten) = rewritten {
                rewritten_sources.insert(template.name.clone(), rewritten);
            }
            sources.insert(template.name, named);
        }

        let catalog = Self {
            sources,
            rewritten_sources,
            origins,
        };
        catalog.validate_references(selected_kit, diagnostics)?;
        Ok(catalog)
    }

    pub(super) fn sources(&self) -> &BTreeMap<TemplateName, NamedSource<String>> {
        &self.sources
    }

    pub(super) fn tera_sources(&self) -> impl Iterator<Item = (&str, &str)> {
        self.sources.iter().map(|(name, source)| {
            let tera_source = self
                .rewritten_sources
                .get(name)
                .map_or_else(|| source.inner().as_str(), String::as_str);
            (name.as_str(), tera_source)
        })
    }

    pub(super) fn into_sources(self) -> BTreeMap<TemplateName, NamedSource<String>> {
        self.sources
    }

    fn validate_references(
        &self,
        selected_kit: Option<&kit::Selected>,
        diagnostics: &DiagnosticPaths,
    ) -> Result<(), Error> {
        let public = selected_kit
            .map(|selected_kit| public_template_names(selected_kit, diagnostics))
            .transpose()?;
        for (template, source) in &self.sources {
            if self.origins[template] == Origin::Root
                && let Some(span) = template_function_call(source.inner(), "__a__")
            {
                return Err(Error::Detailed(Box::new(TemplateDiagnostic {
                    stage: "reference",
                    rendering: String::new(),
                    template: template.to_string(),
                    message: "__a__ is reserved for kit-private asset resolution".into(),
                    source_code: source.clone(),
                    label: "reserved function call here".into(),
                    span,
                    help: Some("use asset() for a public copied asset".into()),
                    related: Vec::new(),
                })));
            }
            for reference in template_references(source.inner()) {
                if !is_kit_reference(reference.name) {
                    if self.origins[template] == Origin::Kit {
                        return Err(reference_error(
                            template,
                            source,
                            reference,
                            format!(
                                "kit template {template:?} may reference only templates in its own kit namespace"
                            ),
                            "use a kit/<name>/… template name for this kit original",
                        ));
                    }
                    continue;
                }

                let Some(selected_kit) = selected_kit else {
                    return Err(reference_error(
                        template,
                        source,
                        reference,
                        "this template names a kit original, but the site selects no kit".into(),
                        "select the kit that provides this template, or replace the reference with a root template",
                    ));
                };
                let expected_prefix = format!("kit/{}/", selected_kit.manifest().name());
                if !reference.name.starts_with(&expected_prefix) {
                    let message = format!(
                        "this template names {name:?}, but the selected kit namespace is {expected_prefix:?}",
                        name = reference.name
                    );
                    return Err(reference_error(
                        template,
                        source,
                        reference,
                        message,
                        "reference a template from the selected kit, or select the kit this reference names",
                    ));
                }

                if self.origins[template] == Origin::Root
                    && !public
                        .as_ref()
                        .is_some_and(|names| names.contains(reference.name))
                {
                    let message = format!(
                        "kit template {name:?} is private and cannot be referenced by a root template",
                        name = reference.name
                    );
                    return Err(reference_error(
                        template,
                        source,
                        reference,
                        message,
                        "reference a declared public kit template, or customize a documented block in one",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Tera intentionally combines every component into one registry. Keep
    /// ownership checks here, while the source origin and manifest spans are
    /// still available, so that registry cannot turn a root definition or
    /// private kit component into an accidental extension point.
    pub(super) fn validate_components(
        &self,
        selected_kit: Option<&kit::Selected>,
    ) -> Result<(), Error> {
        let Some(selected_kit) = selected_kit else {
            return Ok(());
        };
        let manifest_source = selected_kit
            .manifest()
            .source()
            .cloned()
            .ok_or(Error::MissingKitManifestSource)?;
        let prefix = format!("{}.", selected_kit.manifest().name());
        let mut kit_definitions = BTreeSet::<&str>::new();

        for (template, source) in &self.sources {
            for definition in component_definitions(source.inner()) {
                match self.origins[template] {
                    Origin::Kit => {
                        if !definition.name.starts_with(&prefix) {
                            return Err(component_error(
                                template,
                                source,
                                definition.span,
                                format!(
                                    "kit component {:?} must use the kit component namespace {prefix:?}",
                                    definition.name
                                ),
                                "rename the component to use this kit namespace",
                            ));
                        }
                        kit_definitions.insert(definition.name);
                    }
                    Origin::Root if definition.name.starts_with(&prefix) => {
                        return Err(component_error(
                            template,
                            source,
                            definition.span,
                            format!(
                                "root component {:?} occupies the selected kit component namespace {prefix:?}",
                                definition.name
                            ),
                            "rename the root component so it cannot replace a kit component",
                        ));
                    }
                    Origin::Root => {}
                }
            }
        }

        let public = selected_kit
            .manifest()
            .public()
            .components()
            .iter()
            .map(|declaration| declaration.value().as_str())
            .collect::<BTreeSet<_>>();
        for declaration in selected_kit.manifest().public().components() {
            if !kit_definitions.contains(declaration.value().as_str()) {
                return Err(manifest_component_error(
                    selected_kit,
                    &manifest_source,
                    declaration.span(),
                    format!(
                        "declared public kit component {:?} is not defined by this kit",
                        declaration.value()
                    ),
                    "define this component in a kit template, or remove it from [public].components",
                ));
            }
        }

        for (template, source) in &self.sources {
            if self.origins[template] != Origin::Root {
                continue;
            }
            for call in component_calls(source.inner(), &prefix) {
                if kit_definitions.contains(call.name.as_str())
                    && !public.contains(call.name.as_str())
                {
                    return Err(component_error(
                        template,
                        source,
                        call.span,
                        format!(
                            "kit component {:?} is private and cannot be called by a root template",
                            call.name
                        ),
                        "call a manifest-declared public kit component, or customize a public kit template",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) fn markdown_accessible_components(
        &self,
        selected_kit: Option<&kit::Selected>,
    ) -> BTreeSet<String> {
        let public_kit = selected_kit.map_or_else(BTreeSet::new, |kit| {
            kit.manifest()
                .public()
                .components()
                .iter()
                .map(|declaration| declaration.value().as_str().to_owned())
                .collect()
        });
        let mut accessible = BTreeSet::new();
        for (template, source) in &self.sources {
            for definition in component_definitions(source.inner()) {
                if self.origins[template] == Origin::Root || public_kit.contains(definition.name) {
                    accessible.insert(definition.name.to_owned());
                }
            }
        }
        accessible
    }
}

#[derive(Debug)]
struct DiscoveredTemplate {
    path: PathBuf,
    name: TemplateName,
    origin: Origin,
}

fn discover(
    root: &Path,
    origin: Origin,
    diagnostics: &DiagnosticPaths,
) -> Result<Vec<DiscoveredTemplate>, Error> {
    WalkDir::new(root)
        .sort_by_file_name()
        .into_iter()
        .map(|entry| {
            entry.map_err(|source| Error::Walk {
                path: diagnostics.display_path(root),
                source: walk_failure(source, root, diagnostics),
            })
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| {
            if entry.file_type().is_symlink() {
                Err(Error::TemplateSymlink {
                    path: diagnostics.display_path(entry.path()),
                })
            } else {
                Ok(entry)
            }
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            let path = entry.path().to_path_buf();
            let relative = path.strip_prefix(root).map_err(|_| Error::NonUtf8Path {
                path: diagnostics.display_path(&path),
            })?;
            if !crate::source_path::has_safe_components(relative) {
                return Err(Error::NonUtf8Path {
                    path: diagnostics.display_path(&path),
                });
            }
            let name = LogicalPath::from_path(relative)
                .map(TemplateName)
                .ok_or_else(|| Error::NonUtf8Path {
                    path: diagnostics.display_path(&path),
                })?;
            Ok(DiscoveredTemplate { path, name, origin })
        })
        .collect()
}

fn walk_failure(source: walkdir::Error, root: &Path, diagnostics: &DiagnosticPaths) -> WalkFailure {
    let physical_root = root.display().to_string();
    let display_root = diagnostics.display_path(root).display().to_string();
    WalkFailure::new(source.to_string().replace(&physical_root, &display_root))
}

fn kit_template_name(
    kit_name: &kit::KitName,
    template: &TemplateName,
    path: &Path,
    diagnostics: &DiagnosticPaths,
) -> Result<TemplateName, Error> {
    TemplateName::parse(format!("kit/{kit_name}/{template}")).map_err(|_| Error::NonUtf8Path {
        path: diagnostics.display_path(path),
    })
}

fn validate_public_templates(
    selected_kit: &kit::Selected,
    templates: &[DiscoveredTemplate],
    diagnostics: &DiagnosticPaths,
) -> Result<(), Error> {
    let names = templates
        .iter()
        .map(|template| template.name.clone())
        .collect::<BTreeSet<_>>();
    for declaration in selected_kit.manifest().public().templates() {
        let name = kit_template_name(
            selected_kit.manifest().name(),
            &TemplateName(declaration.value().clone()),
            selected_kit.paths().manifest(),
            diagnostics,
        )?;
        if !names.contains(&name) {
            return Err(Error::Detailed(Box::new(TemplateDiagnostic {
                stage: "configuration",
                rendering: String::new(),
                template: declaration.value().as_str().into(),
                message: format!(
                    "declared public kit template {template:?} does not exist",
                    template = declaration.value()
                ),
                source_code: selected_kit
                    .manifest()
                    .source()
                    .cloned()
                    .ok_or(Error::MissingKitManifestSource)?,
                label: "declared public template here".into(),
                span: declaration.span(),
                help: None,
                related: Vec::new(),
            })));
        }
    }
    Ok(())
}

fn public_template_names(
    selected_kit: &kit::Selected,
    diagnostics: &DiagnosticPaths,
) -> Result<BTreeSet<String>, Error> {
    selected_kit
        .manifest()
        .public()
        .templates()
        .iter()
        .map(|declaration| {
            kit_template_name(
                selected_kit.manifest().name(),
                &TemplateName(declaration.value().clone()),
                selected_kit.paths().manifest(),
                diagnostics,
            )
            .map(|name| name.to_string())
        })
        .collect()
}

fn is_kit_namespace(name: &TemplateName) -> bool {
    name.is_kit_namespace()
}

fn is_kit_reference(name: &str) -> bool {
    name == "kit" || name.starts_with("kit/")
}

fn reference_error(
    template: &TemplateName,
    source: &NamedSource<String>,
    reference: TemplateReference<'_>,
    message: String,
    help: &str,
) -> Error {
    Error::Detailed(Box::new(TemplateDiagnostic {
        stage: "reference",
        rendering: String::new(),
        template: template.to_string(),
        message,
        source_code: source.clone(),
        label: "template reference here".into(),
        span: reference.span,
        help: Some(help.into()),
        related: Vec::new(),
    }))
}

fn component_error(
    template: &TemplateName,
    source: &NamedSource<String>,
    span: SourceSpan,
    message: String,
    help: &str,
) -> Error {
    Error::Detailed(Box::new(TemplateDiagnostic {
        stage: "component",
        rendering: String::new(),
        template: template.to_string(),
        message,
        source_code: source.clone(),
        label: "component declaration or call here".into(),
        span,
        help: Some(help.into()),
        related: Vec::new(),
    }))
}

fn manifest_component_error(
    selected_kit: &kit::Selected,
    source_code: &NamedSource<String>,
    span: SourceSpan,
    message: String,
    help: &str,
) -> Error {
    Error::Detailed(Box::new(TemplateDiagnostic {
        stage: "component",
        rendering: String::new(),
        template: selected_kit.paths().manifest().display().to_string(),
        message,
        source_code: source_code.clone(),
        label: "public component declaration here".into(),
        span,
        help: Some(help.into()),
        related: Vec::new(),
    }))
}
