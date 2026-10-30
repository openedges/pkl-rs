//! Actual typed Rust Host completions for the retained finite provider fixtures.
//! Provider decisions are not Java SPI callbacks. The driver retains typed
//! requests/completions separately, including evaluator-issued u64 identities.
use super::glob_fixture::{Provider, Reader, Rule};
use pkl_runtime::glob::{
    GlobCompletion, GlobHostError, GlobReader, GlobRequest, GlobRequestKind, ResourceReader,
    ResourceReaderId,
};
use pkl_runtime::host::{IoFailure, ReadFailure};
use pkl_runtime::modules::{
    self, ModuleCapabilities, ModuleCompletion, ModuleHostError, ModuleKey, ModuleKeyId,
    ModuleKeyKind, ModuleRequest, ModuleRequestKind, ModuleSource, ResolvedModule,
};
use pkl_runtime::probe::SourceProbe;
use pkl_runtime::resources::ReadResult;
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;
use std::collections::BTreeMap;

type Result<T> = std::result::Result<T, String>;

pub struct FixtureHost {
    pub provider: Provider,
    resources: BTreeMap<ResourceReaderId, Reader>,
    modules: BTreeMap<ModuleKeyId, Reader>,
    generic_urls: BTreeMap<ModuleKeyId, ModuleKey>,
}
impl FixtureHost {
    pub fn new(provider: Provider) -> Self {
        Self {
            provider,
            resources: BTreeMap::new(),
            modules: BTreeMap::new(),
            generic_urls: BTreeMap::new(),
        }
    }
    fn generic_url(&self, key: &ModuleKey) -> Result<bool> {
        let Some(selected) = self.generic_urls.get(&key.id) else {
            return Ok(false);
        };
        if selected != key {
            return Err("changed GenericUrl module identity".into());
        }
        Ok(true)
    }
    fn module_reader(&self, key: &ModuleKey) -> Result<Reader> {
        let reader = self
            .modules
            .get(&key.id)
            .ok_or("unknown selected module identity")?;
        if reader.selected_uri != key.uri
            || reader.owner != key.handle
            || key.kind != ModuleKeyKind::Host
        {
            return Err("changed module reader identity".into());
        }
        Ok(reader.clone())
    }
    fn reader(&self, reader: &GlobReader) -> Result<Reader> {
        match reader {
            GlobReader::Module(key) => self.module_reader(key),
            GlobReader::Resource(key) => {
                let reader = self
                    .resources
                    .get(&key.id)
                    .ok_or("unknown selected resource identity")?;
                if reader.owner != key.handle || reader.scheme != key.scheme {
                    return Err("changed resource reader identity".into());
                }
                Ok(reader.clone())
            }
        }
    }
    fn policy(
        &self,
        uri: &PklString,
        resource: bool,
    ) -> Result<std::result::Result<(), ModuleHostError>> {
        let key = if resource {
            "allowedResources"
        } else {
            "allowedModules"
        };
        let patterns = self
            .provider
            .fixture
            .config
            .get(key)
            .and_then(PklString::as_str)
            .ok_or("missing scalar fixture allowlist")?;
        // This is the exact finite policy roster of all 288 authored fixtures.
        // Java Pattern.lookingAt for these patterns is a prefix check, including
        // `scheme:.*` when the remaining URI starts with a line terminator.
        for pattern in patterns.split(',') {
            let prefix = match pattern {
                "gmain:.*" => "gmain:",
                "gdata:.*" => "gdata:",
                "gmod:.*" => "gmod:",
                "pkl:.*" => "pkl:",
                "repl:text" => "repl:text",
                _ => return Err("fixture allowlist outside retained finite pattern roster".into()),
            };
            if uri.starts_with(prefix) {
                return Ok(Ok(()));
            }
        }
        let (verb, noun, option) = if resource {
            ("read", "resource", "resources")
        } else {
            ("load", "module", "modules")
        };
        Ok(Err(ModuleHostError::SecurityManagerException { message: Some(PklString::from(format!("Refusing to {verb} {noun} `")).concat(uri).concat(&format!("` because it does not match any entry in the {noun} allowlist (`--allowed-{option}`).").into())) }))
    }
    fn failure(rule: &Rule) -> Result<Option<ModuleHostError>> {
        Ok(match rule.kind.as_str() {
            "io" => Some(ModuleHostError::Io(IoFailure::new(Some(
                rule.value.clone(),
            )))),
            "security" => Some(ModuleHostError::SecurityManagerException {
                message: Some(rule.value.clone()),
            }),
            "external-reader" => Some(ModuleHostError::ExternalReaderProcess {
                message: Some(rule.value.clone()),
            }),
            "missing" => Some(ModuleHostError::NotFound),
            "value" | "elements" => None,
            "unchecked" => return Err(
                "required unchecked Java callback outcome has no typed adapter representation yet"
                    .into(),
            ),
            _ => return Err("unknown fixture failure kind".into()),
        })
    }
    fn capability_script(
        &mut self,
        operation: &str,
        uri: &PklString,
    ) -> Result<Option<ModuleHostError>> {
        let rule = self.provider.rule(operation, uri)?;
        if let Some(rule) = &rule
            && (operation == "fragment-paths" || rule.kind == "security")
        {
            return Err("unchecked Java capability outcome retained as adapter failure".into());
        }
        rule.as_ref()
            .map(Self::failure)
            .transpose()
            .map(Option::flatten)
    }
    fn module_script(
        &mut self,
        operation: &str,
        uri: &PklString,
    ) -> Result<Option<ModuleHostError>> {
        let rule = self.provider.rule(operation, uri)?;
        if let Some(rule) = &rule
            && (rule.kind == "external-reader"
                || (operation == "load-source" && rule.kind == "security"))
        {
            return Err(
                "unchecked Java module callback outcome retained as adapter failure".into(),
            );
        }
        rule.as_ref()
            .map(Self::failure)
            .transpose()
            .map(Option::flatten)
    }
    pub fn glob(&mut self, request: &GlobRequest) -> Result<GlobCompletion> {
        if let GlobRequestKind::SelectResource { id, uri } = &request.kind {
            let text = SourceText::from(uri);
            let scheme = text
                .entire()
                .find(":")
                .and_then(|end| text.entire().get(..end))
                .ok_or("selected resource URI has no scheme")?
                .to_pkl_string();
            return Ok(GlobCompletion::Resource(
                match self.provider.resource_reader(&scheme)? {
                    Some(reader) => {
                        let result = ResourceReader {
                            id: *id,
                            scheme: reader.scheme.clone(),
                            handle: reader.owner.clone(),
                        };
                        if self.resources.insert(*id, reader).is_some() {
                            return Err("reused resource reader identity".into());
                        }
                        Ok(result)
                    }
                    None => Err(GlobHostError::ReaderNotFound),
                },
            ));
        }
        let (reader, operation) = match &request.kind {
            GlobRequestKind::IsGlobbable { reader } => (reader.clone(), "globbable"),
            GlobRequestKind::IsHierarchical { reader } => (reader.clone(), "hierarchical"),
            GlobRequestKind::FragmentPaths { reader } => {
                (GlobReader::Resource(reader.clone()), "fragment-paths")
            }
            GlobRequestKind::HasElement { reader, .. } => {
                (GlobReader::Resource(reader.clone()), "has-element")
            }
            GlobRequestKind::ListElements { reader, .. } => (reader.clone(), "list-elements"),
            GlobRequestKind::SelectResource { .. } => unreachable!(),
        };
        if let GlobReader::Module(key) = &reader
            && self.generic_url(key)?
        {
            return match &request.kind {
                GlobRequestKind::IsGlobbable { .. } => Ok(GlobCompletion::Globbable(Ok(false))),
                GlobRequestKind::IsHierarchical { .. } => {
                    Ok(GlobCompletion::Hierarchical(Ok(true)))
                }
                _ => Err("GenericUrl is not globbable; unexpected element operation".into()),
            };
        }
        let resource = matches!(reader, GlobReader::Resource(_));
        let reader = self.reader(&reader)?;
        let completion = match &request.kind {
            GlobRequestKind::IsGlobbable { .. }
            | GlobRequestKind::IsHierarchical { .. }
            | GlobRequestKind::FragmentPaths { .. } => {
                let failure = self.capability_script(operation, &reader.selected_uri)?;
                let setting = match operation {
                    "globbable" => "globbable",
                    "hierarchical" => "hierarchical",
                    _ => "fragmentPaths",
                };
                let value = match failure {
                    Some(error) => Err(error.into()),
                    None => Ok(self.provider.boolean(&reader.owner, setting)?),
                };
                match operation {
                    "globbable" => GlobCompletion::Globbable(value),
                    "hierarchical" => GlobCompletion::Hierarchical(value),
                    _ => GlobCompletion::FragmentPaths(value),
                }
            }
            GlobRequestKind::HasElement { uri, .. } => {
                let (rule, exists) = self.provider.exists(uri)?;
                let failure = rule.as_ref().map(Self::failure).transpose()?.flatten();
                GlobCompletion::HasElement(match failure {
                    Some(error) => Err(error.into()),
                    None => self
                        .policy(uri, resource)?
                        .map(|()| exists)
                        .map_err(Into::into),
                })
            }
            GlobRequestKind::ListElements { uri, .. } => {
                let rule = self.provider.rule("list-elements", uri)?;
                let result = match rule.as_ref().map(Self::failure).transpose()?.flatten() {
                    Some(error) => Err(error),
                    None => match self.policy(uri, resource)? {
                        Err(error) => Err(error),
                        Ok(()) => Ok(self.provider.elements_after_admission(
                            &reader,
                            uri,
                            rule.as_ref(),
                        )?),
                    },
                };
                GlobCompletion::Elements(result.map_err(Into::into))
            }
            _ => unreachable!(),
        };
        Ok(completion)
    }
    pub fn module(&mut self, request: &ModuleRequest) -> Result<ModuleCompletion> {
        if let ModuleRequestKind::SelectKey { id, uri, .. } = &request.kind {
            if let Some(reader) = self.provider.module_reader(uri)? {
                let key = ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind: ModuleKeyKind::Host,
                    handle: reader.owner.clone(),
                };
                if self.modules.insert(*id, reader).is_some() {
                    return Err("reused module key identity".into());
                }
                return Ok(ModuleCompletion::KeySelected(Ok(key)));
            }
            if let Some(completion) = modules::default_completion(request) {
                return Ok(completion);
            }
            // Pinned ModuleKeyFactories.GenericUrl is last: absolute hierarchical
            // URIs (or jar:) are selected even when their scheme has no provider.
            // The core already validated the URI before requesting selection.
            let text = SourceText::from(uri);
            let accepted = text.entire().find(":").is_some_and(|colon| {
                let scheme = text.entire().get(..colon).unwrap().to_pkl_string();
                let rest = text.entire().get(colon + 1..).unwrap().to_pkl_string();
                rest.starts_with("/")
                    || scheme
                        .as_str()
                        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("jar"))
            });
            if accepted {
                let key = ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind: ModuleKeyKind::Host,
                    handle: "fixture:java-generic-url".into(),
                };
                if self.modules.contains_key(id)
                    || self.generic_urls.insert(*id, key.clone()).is_some()
                {
                    return Err("reused GenericUrl module key identity".into());
                }
                return Ok(ModuleCompletion::KeySelected(Ok(key)));
            }
            return Ok(ModuleCompletion::KeySelected(Err(
                ModuleHostError::NoLoader,
            )));
        }
        let generic = match &request.kind {
            ModuleRequestKind::QueryCachePolicy { key }
            | ModuleRequestKind::QueryFragmentPaths { key }
            | ModuleRequestKind::Capabilities { key: Some(key), .. } => self.generic_url(key)?,
            _ => false,
        };
        if generic {
            return Ok(match &request.kind {
                ModuleRequestKind::QueryCachePolicy { .. } => {
                    ModuleCompletion::CachePolicy(Ok(true))
                }
                ModuleRequestKind::QueryFragmentPaths { .. } => {
                    ModuleCompletion::FragmentPaths(Ok(false))
                }
                ModuleRequestKind::Capabilities { .. } => {
                    ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                        local: false,
                        hierarchical: true,
                        fragment_paths: false,
                    }))
                }
                _ => unreachable!(),
            });
        }
        let completion = match &request.kind {
            ModuleRequestKind::CheckImport { .. } => modules::default_completion(request)
                .ok_or("default import trust operation unavailable")?,
            ModuleRequestKind::QueryCachePolicy { key } if key.kind == ModuleKeyKind::Host => {
                self.module_reader(key)?;
                ModuleCompletion::CachePolicy(Ok(true))
            }
            ModuleRequestKind::QueryFragmentPaths { key } if key.kind == ModuleKeyKind::Host => {
                let reader = self.module_reader(key)?;
                ModuleCompletion::FragmentPaths(
                    match self.capability_script("fragment-paths", &reader.selected_uri)? {
                        Some(error) => Err(error),
                        None => Ok(self.provider.boolean(&reader.owner, "fragmentPaths")?),
                    },
                )
            }
            ModuleRequestKind::ResolveImportUri { key, authored_uri }
                if key.kind == ModuleKeyKind::Host =>
            {
                let reader = self.module_reader(key)?;
                let result = if let Some(failure) =
                    self.module_script("resolve-uri", authored_uri)?
                {
                    Err(failure)
                } else {
                    // ReaderBase.resolveUri calls hasFragmentPaths for every resolution,
                    // before checking whether the relative URI is absolute.
                    self.capability_script("fragment-paths", &reader.selected_uri)?;
                    let base = if self.provider.fixture.config.get("sourceUri") == Some(&key.uri) {
                        self.provider
                            .fixture
                            .config
                            .get("resolveOverride")
                            .filter(|value| !value.is_empty())
                            .unwrap_or(&key.uri)
                    } else {
                        &key.uri
                    };
                    SourceProbe::resolve_glob_host_uri(
                        base,
                        authored_uri,
                        self.provider.boolean(&reader.owner, "fragmentPaths")?,
                    )
                    .map_err(|failure| ModuleHostError::Unsupported(failure.message))
                };
                ModuleCompletion::ImportUri(result)
            }
            ModuleRequestKind::Capabilities { key: Some(key), .. }
                if key.kind == ModuleKeyKind::Host =>
            {
                let reader = self.module_reader(key)?;
                // This bundled Rust effect carries values, not synthesized Java SPI calls.
                // Java resolveTripleDotImport short-circuits at isLocal=false for these
                // fixtures, so it must not consume hierarchical/fragment failure scripts.
                ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                    local: false,
                    hierarchical: self.provider.boolean(&reader.owner, "hierarchical")?,
                    fragment_paths: self.provider.boolean(&reader.owner, "fragmentPaths")?,
                }))
            }
            ModuleRequestKind::HasElement {
                key: Some(key),
                element_uri,
                ..
            } if key.kind == ModuleKeyKind::Host => {
                self.module_reader(key)?;
                let (rule, exists) = self.provider.exists(element_uri)?;
                ModuleCompletion::HasElement(
                    match rule.as_ref().map(Self::failure).transpose()?.flatten() {
                        Some(error) => Err(error),
                        None => self.policy(element_uri, false)?.map(|()| exists),
                    },
                )
            }
            ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. }
                if key.kind == ModuleKeyKind::Host =>
            {
                self.module_reader(key)?;
                let result = if let Some(failure) = self.module_script("resolve", &key.uri)? {
                    Err(failure)
                } else if let Err(failure) = self.policy(&key.uri, false)? {
                    Err(failure)
                } else {
                    let resolved =
                        if self.provider.fixture.config.get("sourceUri") == Some(&key.uri) {
                            self.provider
                                .fixture
                                .config
                                .get("resolvedPrimary")
                                .filter(|value| !value.is_empty())
                                .cloned()
                        } else {
                            None
                        };
                    let resolved = match resolved {
                        Some(uri) => Ok(uri),
                        None => SourceProbe::normalize_glob_host_uri(&key.uri)
                            .map_err(|failure| ModuleHostError::Unsupported(failure.message)),
                    };
                    resolved.map(|uri| ResolvedModule {
                        original_uri: key.uri.clone(),
                        resolved_uri: uri.clone(),
                        source_uri: uri,
                    })
                };
                ModuleCompletion::Resolved(result)
            }
            ModuleRequestKind::LoadSource { key, resolved }
                if key.key.kind == ModuleKeyKind::Host =>
            {
                self.module_reader(&key.key)?;
                let result = match self.provider.module_source(&resolved.resolved_uri)? {
                    Ok(text) => Ok(text),
                    Err(rule) if rule.kind == "value" => Ok(scripted_module_source(&rule.value)),
                    Err(rule) if matches!(rule.kind.as_str(), "security" | "external-reader") => {
                        return Err(
                            "unchecked Java load callback outcome retained as adapter failure"
                                .into(),
                        );
                    }
                    Err(rule) => Err(Self::failure(&rule)?
                        .ok_or("module source rule did not produce failure")?),
                };
                ModuleCompletion::Source(result.map(|text| ModuleSource {
                    text,
                    inferred_name: PklString::default(),
                }))
            }
            _ => modules::default_completion(request)
                .ok_or("unconfigured module effect (retained as adapter failure)")?,
        };
        Ok(completion)
    }
    pub fn read(
        &mut self,
        uri: &PklString,
    ) -> Result<std::result::Result<ReadResult, ReadFailure>> {
        // ResourceManager applies security before missing-reader admission and
        // before reader.read; the evaluator already supplies normalized URI.
        if let Err(failure) = self.policy(uri, true)? {
            let ModuleHostError::SecurityManagerException { message } = failure else {
                unreachable!()
            };
            return Ok(Err(ReadFailure::SecurityManagerException { message }));
        }
        let text = SourceText::from(uri);
        let scheme = text
            .entire()
            .find(":")
            .and_then(|end| text.entire().get(..end))
            .ok_or("resource URI lacks scheme")?
            .to_pkl_string();
        self.provider.resource_reader(&scheme)?.ok_or(
            "required ordinary missing-reader failure has no typed read representation yet",
        )?;
        Ok(match self.provider.resource(uri)? {
            Ok(value) => Ok(ReadResult::Text(value)),
            Err(rule) => match rule.kind.as_str() {
                "missing" => Ok(ReadResult::Missing),
                "value" => Ok(ReadResult::Text(rule.value)),
                "io" => Err(ReadFailure::Io(IoFailure::new(Some(rule.value)))),
                "security" => Err(ReadFailure::SecurityManagerException {
                    message: Some(rule.value),
                }),
                "external-reader" => return Err(
                    "required external-reader value failure has no typed read representation yet"
                        .into(),
                ),
                "unchecked" => {
                    return Err(
                        "required unchecked read outcome has no typed adapter representation yet"
                            .into(),
                    );
                }
                _ => return Err("invalid scripted resource response".into()),
            },
        })
    }
}

fn scripted_module_source(value: &PklString) -> SourceText {
    // Exact Java fixture `pkl` constructor: raw surrogate units are left alone.
    // This builds authored fixture source, never rewrites loaded source text.
    let mut units: Vec<u16> = "name = \"".encode_utf16().collect();
    for unit in value.encode_utf16() {
        match unit {
            0x5c => units.extend([0x5c, 0x5c]),
            0x22 => units.extend([0x5c, 0x22]),
            0x0a => units.extend([0x5c, 0x6e]),
            0x0d => units.extend([0x5c, 0x72]),
            _ => units.push(unit),
        }
    }
    units.extend("\"\n".encode_utf16());
    SourceText::from(PklString::from_utf16(units))
}

#[cfg(test)]
mod tests {
    use super::super::glob_fixture::Fixture;
    use super::*;
    use std::collections::BTreeSet;

    fn host(rules: Vec<Rule>) -> FixtureHost {
        FixtureHost::new(Provider::new(Fixture {
            config: [
                ("allowedResources".into(), "gdata:.*".into()),
                ("resourceSchemes".into(), "gdata".into()),
                ("moduleProviderSchemes".into(), "gmod".into()),
                ("hierarchical".into(), "true".into()),
                ("fragmentPaths".into(), "false".into()),
            ]
            .into_iter()
            .collect(),
            listings: BTreeMap::new(),
            modules: BTreeMap::new(),
            resources: BTreeMap::new(),
            present: BTreeSet::new(),
            rules,
            owners: Vec::new(),
            queries: Vec::new(),
            source: "".into(),
        }))
    }
    fn rule(operation: &str, uri: &str, kind: &str, value: PklString) -> Rule {
        Rule {
            operation: operation.into(),
            uri: uri.into(),
            attempt: 1,
            kind: kind.into(),
            value,
        }
    }
    #[test]
    fn listing_script_precedes_policy_and_policy_precedes_unconfigured_table() {
        let raw = PklString::from_utf16(vec![0xd800, 0x78, 0xdfff]);
        let mut host = host(vec![rule(
            "list-elements",
            "denied:/first",
            "io",
            raw.clone(),
        )]);
        let GlobCompletion::Resource(Ok(reader)) = host
            .glob(&GlobRequest {
                id: 1,
                kind: GlobRequestKind::SelectResource {
                    id: ResourceReaderId(u64::MAX - 1),
                    uri: "gdata:/tree/*".into(),
                },
            })
            .unwrap()
        else {
            panic!("configured reader");
        };
        assert_eq!(reader.id, ResourceReaderId(u64::MAX - 1));
        let listing = |uri: &str| GlobRequest {
            id: 2,
            kind: GlobRequestKind::ListElements {
                reader: GlobReader::Resource(reader.clone()),
                uri: uri.into(),
            },
        };
        assert_eq!(
            host.glob(&listing("denied:/first")).unwrap(),
            GlobCompletion::Elements(Err(ModuleHostError::Io(IoFailure::new(Some(raw))).into()))
        );
        assert_eq!(host.glob(&listing("denied:/second")).unwrap(), GlobCompletion::Elements(Err(ModuleHostError::SecurityManagerException {
            message: Some("Refusing to read resource `denied:/second` because it does not match any entry in the resource allowlist (`--allowed-resources`).".into())
        }.into())));
        assert!(host.glob(&listing("gdata:/unconfigured")).is_err());
        let mut changed = reader;
        changed.handle = "resource:gdata:other".into();
        assert_eq!(
            host.glob(&GlobRequest {
                id: 3,
                kind: GlobRequestKind::IsGlobbable {
                    reader: GlobReader::Resource(changed)
                }
            })
            .unwrap_err(),
            "changed resource reader identity"
        );
    }
    #[test]
    fn triple_dot_capabilities_do_not_consume_skipped_java_getters() {
        let raw = PklString::from_utf16(vec![0xdfff]);
        let mut host = host(vec![
            rule("hierarchical", "gmod:/base", "io", raw.clone()),
            rule("fragment-paths", "gmod:/base", "unchecked", "getter".into()),
        ]);
        let ModuleCompletion::KeySelected(Ok(key)) = host
            .module(&ModuleRequest {
                id: 1,
                kind: ModuleRequestKind::SelectKey {
                    id: ModuleKeyId(u64::MAX),
                    uri: "gmod:/base".into(),
                    phase: modules::SelectionPhase::Primary,
                },
            })
            .unwrap()
        else {
            panic!("configured module key");
        };
        assert_eq!(
            host.module(&ModuleRequest {
                id: 2,
                kind: ModuleRequestKind::Capabilities {
                    key: Some(key.clone()),
                    module: modules::ModuleIdentity {
                        original_uri: key.uri.clone(),
                        resolved_uri: key.uri.clone()
                    }
                }
            })
            .unwrap(),
            ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                local: false,
                hierarchical: true,
                fragment_paths: false
            }))
        );
        assert!(host.provider.decisions.is_empty());
        assert_eq!(
            host.glob(&GlobRequest {
                id: 3,
                kind: GlobRequestKind::IsHierarchical {
                    reader: GlobReader::Module(key.clone())
                }
            })
            .unwrap(),
            GlobCompletion::Hierarchical(
                Err(ModuleHostError::Io(IoFailure::new(Some(raw))).into())
            )
        );
        // IoUtils.resolve invokes the fragment getter even for absolute relative URIs.
        assert_eq!(
            host.module(&ModuleRequest {
                id: 4,
                kind: ModuleRequestKind::ResolveImportUri {
                    key,
                    authored_uri: "gmod:/absolute".into()
                }
            })
            .unwrap_err(),
            "unchecked Java capability outcome retained as adapter failure"
        );
        assert_eq!(host.provider.decisions.len(), 2);
    }
    #[test]
    fn generic_url_selection_keeps_identity_and_never_becomes_a_globbable_default() {
        let mut host = host(Vec::new());
        let select = |id, uri: &str| ModuleRequest {
            id,
            kind: ModuleRequestKind::SelectKey {
                id: ModuleKeyId(id),
                uri: uri.into(),
                phase: modules::SelectionPhase::Primary,
            },
        };
        for (id, uri) in [
            (71, "x-y:/tree/*"),
            (72, "unregistered:/tree/*"),
            (73, "JAR:file:/a.jar!/entry"),
        ] {
            let ModuleCompletion::KeySelected(Ok(key)) = host.module(&select(id, uri)).unwrap()
            else {
                panic!("GenericUrl selected");
            };
            assert_eq!(key.uri, uri);
            assert_eq!(key.handle, "fixture:java-generic-url");
            assert_eq!(key.id, ModuleKeyId(id));
            let glob = |key| GlobRequest {
                id: 80,
                kind: GlobRequestKind::IsGlobbable {
                    reader: GlobReader::Module(key),
                },
            };
            assert_eq!(
                host.glob(&glob(key.clone())).unwrap(),
                GlobCompletion::Globbable(Ok(false))
            );
            let mut forged = key.clone();
            forged.uri = "x-y:/another".into();
            assert_eq!(
                host.glob(&glob(forged)).unwrap_err(),
                "changed GenericUrl module identity"
            );
            let mut forged = key.clone();
            forged.handle = "module:x-y:provider-1".into();
            assert_eq!(
                host.glob(&glob(forged)).unwrap_err(),
                "changed GenericUrl module identity"
            );
            assert!(
                host.glob(&GlobRequest {
                    id: 81,
                    kind: GlobRequestKind::ListElements {
                        reader: GlobReader::Module(key),
                        uri: uri.into()
                    }
                })
                .is_err()
            );
        }
        assert_eq!(
            host.module(&select(74, "unregistered:opaque")).unwrap(),
            ModuleCompletion::KeySelected(Err(ModuleHostError::NoLoader))
        );
        let ModuleCompletion::KeySelected(Ok(key)) =
            host.module(&select(75, "gmod:/tree/*")).unwrap()
        else {
            panic!("registered custom provider");
        };
        assert_ne!(key.handle, "fixture:java-generic-url");
        assert!(host.modules.contains_key(&key.id));
        assert!(!host.generic_urls.contains_key(&key.id));
        assert!(
            host.provider.decisions.is_empty(),
            "selection/getters must not invoke listing or value scripts"
        );
    }
}
