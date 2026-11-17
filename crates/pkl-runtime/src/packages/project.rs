//! Pure PklProject dependency structures and JSON deps modeling.
//!
//! Filesystem reading and process execution are delegated to the Host adapter outside `pkl-runtime`.
use crate::modules::ModuleHostError;
use crate::packages::PackageUri;
use crate::packages::metadata::{Issue, Node, parse_json};
use crate::value::PklString;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedProjectDep {
    pub dep_type: String, // "local" or "remote"
    pub uri: PklString,   // "projectpackage://..."
    pub path: Option<String>,
    pub sha256: Option<PklString>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EvaluatorSettings {
    pub external_properties: Vec<(String, String)>,
    pub env: Vec<(String, String)>,
    pub allowed_modules: Option<Vec<String>>,
    pub allowed_resources: Option<Vec<String>>,
    pub no_cache: Option<bool>,
    pub module_cache_dir: Option<PathBuf>,
    pub root_dir: Option<PathBuf>,
    pub trace_mode: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredDep {
    pub name: String,
    pub uri: String,
    pub is_local: bool,
    pub local_package_uri: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PklProject {
    pub project_dir: PathBuf,
    pub dependencies: BTreeMap<String, ResolvedProjectDep>,
    pub declared_dependencies: BTreeMap<String, DeclaredDep>,
    pub deps_loaded: bool,
}

pub fn compare_dep_versions(v1: &str, v2: &str) -> std::cmp::Ordering {
    let parse_parts = |s: &str| -> Vec<u64> {
        s.split('.')
            .map(|seg| {
                seg.split('-')
                    .next()
                    .unwrap_or(seg)
                    .parse::<u64>()
                    .unwrap_or(0)
            })
            .collect()
    };
    let p1 = parse_parts(v1);
    let p2 = parse_parts(v2);
    p1.cmp(&p2)
}

pub fn extract_version_from_uri(uri: &str) -> Option<&str> {
    uri.rsplit('@').next()
}

/// Parse PklProject.deps.json content into resolved dependencies.
pub fn parse_deps_json(
    text: &str,
    resolved: &mut BTreeMap<String, ResolvedProjectDep>,
    deps_path_display: &str,
) -> Result<(), ModuleHostError> {
    let node = parse_json(&PklString::from(text)).map_err(|err| {
        let err_str = match &err {
            Issue::Java(s) | Issue::Unsupported(s) => s.as_str().unwrap_or(""),
        };
        let msg = format!(
            "Cannot resolve dependency because file `{deps_path_display}` is malformed.\nRun `pkl project resolve` to re-create this file."
        );
        ModuleHostError::EvalError {
            message: msg.into(),
            hint: Some(format!("Details: {err_str}").into()),
        }
    })?;

    let Node::Object(root) = node else {
        let msg = format!(
            "Cannot resolve dependency because file `{deps_path_display}` is malformed.\nRun `pkl project resolve` to re-create this file."
        );
        return Err(ModuleHostError::EvalError {
            message: msg.into(),
            hint: Some("Details: Expected JSON object at root.".into()),
        });
    };

    let deps_val = root
        .entries
        .iter()
        .find(|(k, _)| k.as_str() == Some("resolvedDependencies"))
        .map(|(_, v)| v);

    let Some(deps_val) = deps_val else {
        return Ok(());
    };

    let Node::Object(deps_obj) = deps_val else {
        let msg = format!(
            "Cannot resolve dependency because file `{deps_path_display}` is malformed.\nRun `pkl project resolve` to re-create this file."
        );
        return Err(ModuleHostError::EvalError {
            message: msg.into(),
            hint: Some("Details: Expected object for `resolvedDependencies`.".into()),
        });
    };

    for (key_str, val) in &deps_obj.entries {
        let Node::Object(dep_obj) = val else {
            continue;
        };

        let key = key_str.as_str().unwrap_or("").to_string();

        let dep_type = dep_obj
            .entries
            .iter()
            .find(|(k, _)| k.as_str() == Some("type"))
            .and_then(|(_, n)| match n {
                Node::String(s) => s.as_str().map(str::to_string),
                _ => None,
            })
            .unwrap_or_else(|| "remote".to_string());

        let uri_str = dep_obj
            .entries
            .iter()
            .find(|(k, _)| k.as_str() == Some("uri"))
            .and_then(|(_, n)| match n {
                Node::String(s) => s.as_str(),
                _ => None,
            })
            .unwrap_or("");

        // Validate URI syntax matching upstream Pkl
        if !uri_str.is_empty()
            && let Err(failure) = PackageUri::parse(uri_str)
        {
            let reason = failure.reason();
            let reason_str = reason.as_str().unwrap_or("");
            let msg = format!(
                "Cannot resolve dependency because file `{deps_path_display}` is malformed.\nRun `pkl project resolve` to re-create this file."
            );
            return Err(ModuleHostError::EvalError {
                message: msg.into(),
                hint: Some(format!("Details: Failed to parse key `resolvedDependencies`: Failed to parse key `uri`: {reason_str}: {uri_str}..").into()),
            });
        }

        let uri = PklString::from(uri_str);

        let path = dep_obj
            .entries
            .iter()
            .find(|(k, _)| k.as_str() == Some("path"))
            .and_then(|(_, n)| match n {
                Node::String(s) => s.as_str().map(str::to_string),
                _ => None,
            });

        let sha256 = dep_obj
            .entries
            .iter()
            .find(|(k, _)| k.as_str() == Some("checksums"))
            .and_then(|(_, n)| match n {
                Node::Object(c) => c
                    .entries
                    .iter()
                    .find(|(k, _)| k.as_str() == Some("sha256"))
                    .map(|(_, v)| v),
                _ => None,
            })
            .and_then(|n| match n {
                Node::String(s) => Some(s.clone()),
                _ => None,
            });

        resolved.insert(
            key,
            ResolvedProjectDep {
                dep_type,
                uri,
                path,
                sha256,
            },
        );
    }

    Ok(())
}

/// Matches a declared dependency URI/name against the resolved dependencies table.
pub fn find_matching_resolved(
    uri: &str,
    name: &str,
    resolved: &BTreeMap<String, ResolvedProjectDep>,
) -> Option<ResolvedProjectDep> {
    // 1. Try matching by exact name
    if let Some(dep) = resolved.get(name) {
        return Some(dep.clone());
    }
    // 2. Try exact URI
    if let Some(dep) = resolved.get(uri) {
        return Some(dep.clone());
    }
    // 3. Try prefix (e.g. package://.../foo@0 for package://.../foo@0.5.0)
    for (k, dep) in resolved {
        if uri.starts_with(k) {
            return Some(dep.clone());
        }
        if let (Some((k_base, _)), Some((uri_base, _))) = (k.split_once('@'), uri.split_once('@'))
            && k_base == uri_base
        {
            return Some(dep.clone());
        }
    }
    None
}

/// Resolves an `@dep` import authored in a project module.
pub fn resolve_project_dependency(
    project: &PklProject,
    authored_uri: &str,
) -> Result<PklString, ModuleHostError> {
    let stripped = authored_uri.strip_prefix('@').unwrap_or(authored_uri);
    let (dep_name, asset_path) = match stripped.find('/') {
        Some(idx) => (&stripped[..idx], &stripped[idx..]),
        None => (stripped, "/"),
    };

    let declared = project.declared_dependencies.get(dep_name);

    if let Some(dep) = project.dependencies.get(dep_name) {
        let base_uri = dep.uri.as_str().unwrap_or("");
        // Version check if declared dependency is available
        if let Some(decl) = declared {
            let project_file_uri = format!("file://{}/PklProject", project.project_dir.display());
            let resolved_uri_clean = base_uri.replace("projectpackage:", "package:");
            if decl.is_local {
                if let Some(local_uri) = &decl.local_package_uri {
                    let dec_ver = extract_version_from_uri(local_uri);
                    let res_ver = extract_version_from_uri(base_uri);
                    if let (Some(dv), Some(rv)) = (dec_ver, res_ver)
                        && dv != rv
                    {
                        let msg = format!(
                            "Project `{project_file_uri}` declares a dependency on a local package whose version doesn't match what is declared in `PklProject.deps.json`.\n\nDeclared: `{local_uri}`\nResolved: `{resolved_uri_clean}`\n\nRun `pkl project resolve` to update resolved dependencies."
                        );
                        return Err(ModuleHostError::SecurityManagerException {
                            message: Some(msg.into()),
                        });
                    }
                }
            } else {
                let dec_ver = extract_version_from_uri(&decl.uri);
                let res_ver = extract_version_from_uri(base_uri);
                if let (Some(dv), Some(rv)) = (dec_ver, res_ver)
                    && compare_dep_versions(dv, rv) == std::cmp::Ordering::Greater
                {
                    let decl_uri = &decl.uri;
                    let msg = format!(
                        "Project `{project_file_uri}` declares a dependency on a package that is newer than what is declared in `PklProject.deps.json`.\n\nDeclared: `{decl_uri}`\nResolved: `{resolved_uri_clean}`\n\nRun `pkl project resolve` to update resolved dependencies."
                    );
                    return Err(ModuleHostError::SecurityManagerException {
                        message: Some(msg.into()),
                    });
                }
            }
        }
        let formatted = format!("{base_uri}#{asset_path}");
        Ok(formatted.into())
    } else if let Some(decl) = declared {
        let declared_uri = &decl.uri;
        let msg = format!(
            "Did not find a resolved dependency for package `{declared_uri}`.\n\nRun `pkl project resolve` to update resolved dependencies."
        );
        Err(ModuleHostError::SecurityManagerException {
            message: Some(msg.into()),
        })
    } else {
        let msg = format!(
            "Cannot find a dependency named `{dep_name}`, because it is not declared in the current project.\n\nTo fix this, add it to the `dependencies` section of your `PklProject` file, and resolve your dependencies."
        );
        Err(ModuleHostError::SecurityManagerException {
            message: Some(msg.into()),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectValidationResult {
    pub is_project: bool,
    pub is_module: bool,
    pub got_type: String,
    pub package_uri: Option<String>,
    pub declared_dependencies: BTreeMap<String, DeclaredDep>,
    pub resolved_evaluator_settings: Option<EvaluatorSettings>,
}

pub fn parse_project_validation(
    json_text: &str,
) -> Result<ProjectValidationResult, ModuleHostError> {
    let node = parse_json(&PklString::from(json_text)).map_err(|err| {
        ModuleHostError::SecurityManagerException {
            message: Some(format!("Failed to parse project JSON: {err:?}").into()),
        }
    })?;

    let Node::Object(root) = node else {
        return Err(ModuleHostError::SecurityManagerException {
            message: Some("Expected JSON object from project validation".into()),
        });
    };

    let is_project = match root.optional("isProject") {
        Some(Node::Boolean(b)) => *b,
        _ => false,
    };
    let is_module = match root.optional("isModule") {
        Some(Node::Boolean(b)) => *b,
        _ => false,
    };
    let got_type = match root.optional("gotType") {
        Some(Node::String(s)) => s.as_str().unwrap_or("Module").to_string(),
        _ => "Module".to_string(),
    };
    let package_uri = match root.optional("packageUri") {
        Some(Node::String(s)) => s.as_str().map(|v| v.to_string()),
        _ => None,
    };

    let mut declared_dependencies = BTreeMap::new();
    if let Some(Node::Object(deps_obj)) = root.optional("declaredDependencies") {
        for (name_str, dep_node) in &deps_obj.entries {
            if let Node::Object(dep_fields) = dep_node {
                let dep_name = name_str.as_str().unwrap_or("").to_string();
                let dep_uri = match dep_fields.optional("uri") {
                    Some(Node::String(s)) => s.as_str().unwrap_or("").to_string(),
                    _ => String::new(),
                };
                let is_local = match dep_fields.optional("isLocal") {
                    Some(Node::Boolean(b)) => *b,
                    _ => false,
                };
                let local_pkg_uri = match dep_fields.optional("localPackageUri") {
                    Some(Node::String(s)) => s.as_str().map(|v| v.to_string()),
                    _ => None,
                };
                declared_dependencies.insert(
                    dep_name.clone(),
                    DeclaredDep {
                        name: dep_name,
                        uri: dep_uri,
                        is_local,
                        local_package_uri: local_pkg_uri,
                    },
                );
            }
        }
    }

    let mut resolved_evaluator_settings = None;
    if let Some(Node::Object(s_obj)) = root.optional("resolvedEvaluatorSettings") {
        let mut s = EvaluatorSettings::default();
        if let Some(Node::Object(env_obj)) = s_obj.optional("env") {
            for (k, v) in &env_obj.entries {
                if let Node::String(vs) = v
                    && let (Some(ks), Some(vss)) = (k.as_str(), vs.as_str())
                {
                    s.env.push((ks.to_string(), vss.to_string()));
                }
            }
        }
        if let Some(Node::Object(props_obj)) = s_obj.optional("externalProperties") {
            for (k, v) in &props_obj.entries {
                if let Node::String(vs) = v
                    && let (Some(ks), Some(vss)) = (k.as_str(), vs.as_str())
                {
                    s.external_properties
                        .push((ks.to_string(), vss.to_string()));
                }
            }
        }
        if let Some(Node::Array(arr)) = s_obj.optional("allowedModules") {
            let mut list = Vec::new();
            for item in arr {
                if let Node::String(str_val) = item
                    && let Some(sv) = str_val.as_str()
                {
                    list.push(sv.to_string());
                }
            }
            s.allowed_modules = Some(list);
        }
        if let Some(Node::Array(arr)) = s_obj.optional("allowedResources") {
            let mut list = Vec::new();
            for item in arr {
                if let Node::String(str_val) = item
                    && let Some(sv) = str_val.as_str()
                {
                    list.push(sv.to_string());
                }
            }
            s.allowed_resources = Some(list);
        }
        if let Some(Node::Boolean(b)) = s_obj.optional("noCache") {
            s.no_cache = Some(*b);
        }
        if let Some(Node::String(str_val)) = s_obj.optional("rootDir")
            && let Some(sv) = str_val.as_str()
        {
            let path_part = sv
                .strip_prefix("file://")
                .or_else(|| sv.strip_prefix("file:"))
                .unwrap_or(sv);
            s.root_dir = Some(PathBuf::from(path_part));
        }
        if let Some(Node::String(str_val)) = s_obj.optional("moduleCacheDir")
            && let Some(sv) = str_val.as_str()
        {
            let path_part = sv
                .strip_prefix("file://")
                .or_else(|| sv.strip_prefix("file:"))
                .unwrap_or(sv);
            s.module_cache_dir = Some(PathBuf::from(path_part));
        }
        if let Some(Node::String(str_val)) = s_obj.optional("traceMode")
            && let Some(sv) = str_val.as_str()
        {
            s.trace_mode = Some(sv.to_string());
        }
        resolved_evaluator_settings = Some(s);
    }

    Ok(ProjectValidationResult {
        is_project,
        is_module,
        got_type,
        package_uri,
        declared_dependencies,
        resolved_evaluator_settings,
    })
}
