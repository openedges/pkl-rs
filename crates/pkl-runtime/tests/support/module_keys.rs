//! Explicit stateless test factory. Test closures supply source/resolve behavior separately.
#![allow(dead_code)]
use pkl_runtime::{modules::*, probe::SourceProbe};
pub fn defaults(request: &ModuleRequest) -> Option<ModuleCompletion> {
    default_completion(request).or_else(|| match &request.kind {
        ModuleRequestKind::SelectKey { id, uri, .. } => {
            Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                id: *id,
                uri: uri.clone(),
                kind: ModuleKeyKind::Host,
                handle: "fixture".into(),
            })))
        }
        ModuleRequestKind::QueryCachePolicy { .. } => Some(ModuleCompletion::CachePolicy(Ok(true))),
        _ => None,
    })
}
pub fn complete(request: &ModuleRequest) -> ModuleCompletion {
    defaults(request).expect("test factory/cache-policy request")
}
pub fn complete_probe(
    probe: &mut SourceProbe,
    token: &str,
    now: u32,
) -> Result<bool, pkl_runtime::value::RuntimeError> {
    let request = probe.module_request(token)?;
    let Some(completion) = defaults(&request) else {
        return Ok(false);
    };
    probe.complete_module(token, completion, now)?;
    Ok(true)
}
