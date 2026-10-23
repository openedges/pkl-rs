//! Bounded captured File Host policy. No URI search, language or cache in this adapter.
use pkl_runtime::{
    host::ReadFailure,
    modules::{
        ModuleCapabilities, ModuleCompletion, ModuleHostError, ModuleRequest, ModuleRequestKind,
    },
    resources::ReadResult,
    value::PklString,
    vm::ReadRequest,
};
use std::{fs, path::Path};

pub struct Observer<'a> {
    out: &'a Path,
    mode: &'a str,
    denied_uri: &'a str,
    module_cause: PklString,
    resource_cause: PklString,
    events: usize,
    pub reads: usize,
    module_requests: usize,
}
fn units(path: &Path) -> PklString {
    let bytes = fs::read(path).expect("captured policy cause units");
    assert_eq!(bytes.len() % 2, 0);
    PklString::from_utf16(
        bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|p| u16::from_be_bytes([p[0], p[1]]))
            .collect::<Vec<_>>(),
    )
}
impl<'a> Observer<'a> {
    pub fn new(
        out: &'a Path,
        mode: &'a str,
        denied_uri: &'a str,
        module_cause: &Path,
        resource_cause: &Path,
    ) -> Self {
        Self {
            out,
            mode,
            denied_uri,
            module_cause: units(module_cause),
            resource_cause: units(resource_cause),
            events: 0,
            reads: 0,
            module_requests: 0,
        }
    }
    pub fn event(&mut self, value: String) {
        fs::write(
            self.out.join(format!("host-event-{}.bin", self.events)),
            value,
        )
        .expect("Host event");
        self.events += 1;
    }
    pub fn counters(&self) -> String {
        format!(
            "read={}\nmoduleRequest={}\nevent={}\n",
            self.reads, self.module_requests, self.events
        )
    }
    fn module_policy(&mut self, uri: &str) -> Result<(), ModuleHostError> {
        self.event(format!("policy-resolve-module {uri}"));
        if self.mode == "deny-module" && uri == self.denied_uri {
            self.event(format!("actual-module-denied {uri}"));
            Err(ModuleHostError::SecurityManagerException {
                message: Some(self.module_cause.clone()),
            })
        } else {
            Ok(())
        }
    }
    pub fn module(&mut self, request: &ModuleRequest) -> ModuleCompletion {
        self.module_requests += 1;
        match &request.kind {
            ModuleRequestKind::ResolveKey { .. } => super::module_host::complete(request),
            ModuleRequestKind::SelectKey { .. }
            | ModuleRequestKind::QueryCachePolicy { .. }
            | ModuleRequestKind::QueryFragmentPaths { .. }
            | ModuleRequestKind::ResolveImportUri { .. } => super::module_host::complete(request),
            ModuleRequestKind::CheckImport {
                importer,
                import_uri,
            } => {
                self.event(format!(
                    "module-check-import {} -> {import_uri}",
                    importer.resolved_uri
                ));
                super::module_host::complete(request)
            }
            ModuleRequestKind::Capabilities { module, .. } => {
                self.event(format!("module-capabilities {}", module.original_uri));
                if self.mode == "cap-nonlocal" || self.mode == "cap-nonhierarchical" {
                    ModuleCompletion::Capabilities(Ok(ModuleCapabilities {
                        local: self.mode != "cap-nonlocal",
                        hierarchical: self.mode != "cap-nonhierarchical",
                        fragment_paths: false,
                    }))
                } else {
                    super::module_host::complete(request)
                }
            }
            ModuleRequestKind::HasElement {
                module,
                element_uri,
                ..
            } => {
                self.event(format!(
                    "module-has-element {} -> {element_uri}",
                    module.original_uri
                ));
                match self.module_policy(element_uri) {
                    Ok(()) => super::module_host::complete(request),
                    Err(e) => ModuleCompletion::HasElement(Err(e)),
                }
            }
            ModuleRequestKind::Resolve {
                importer,
                import_uri,
                original_uri,
                ..
            } => {
                self.event(format!(
                    "module-resolve {} -> {import_uri}",
                    importer.original_uri
                ));
                self.event(format!(
                    "policy-import {} -> {original_uri}",
                    importer.original_uri
                ));
                match self.module_policy(original_uri) {
                    Ok(()) => {
                        self.event(format!("secure-path {original_uri} resource=false"));
                        super::module_host::complete(request)
                    }
                    Err(e) => ModuleCompletion::Resolved(Err(e)),
                }
            }
            ModuleRequestKind::LoadSource { resolved, .. } => {
                self.event(format!("module-source {}", resolved.original_uri));
                match self.module_policy(&resolved.original_uri) {
                    Ok(()) => super::module_host::complete(request),
                    Err(_) => ModuleCompletion::Source(Err(ModuleHostError::Unsupported("Source-stage security denial is outside this captured resolution-only contract.".into()))),
                }
            }
        }
    }
    pub fn read(&mut self, request: &ReadRequest) -> Option<Result<ReadResult, ReadFailure>> {
        let uri = request.resolved_uri().expect("shared resolved reader URI");
        self.event(format!("reader-request {uri}"));
        // This graph has file resources. Other schemes retain configured-runtime behavior.
        if !uri.starts_with("file:") {
            return None;
        }
        self.event(format!("policy-read-resource {uri}"));
        if self.mode == "deny-resource" && uri == self.denied_uri {
            self.event(format!("actual-resource-denied {uri}"));
            return Some(Err(ReadFailure::SecurityManagerException {
                message: Some(self.resource_cause.clone()),
            }));
        }
        self.event(format!("actual-file-reader {uri}"));
        fs::write(
            self.out.join(format!("read-{}.uri.bin", self.reads)),
            uri.as_bytes(),
        )
        .expect("reader trace");
        self.reads += 1;
        self.event(format!("secure-path {uri} resource=true"));
        super::resource_host::complete(request)
    }
}
