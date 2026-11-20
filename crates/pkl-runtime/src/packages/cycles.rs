//! Pure circular local-project dependency detection and rendering.
//!
//! Matches `org.pkl.core.util.ImportGraphUtils` (`ImportGraphUtils.java:29-65`) and
//! `org.pkl.core.project.Project` (`Project.java:149-236`).
use std::collections::BTreeMap;

/// Finds import cycles in an ordered adjacency graph (`uri -> ordered imported uris`),
/// matching `ImportGraphUtils.findImportCycles` (`ImportGraphUtils.java:29-65`).
pub fn find_import_cycles(imports: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
    let mut cycles: Vec<Vec<String>> = Vec::new();
    for uri in imports.keys() {
        if cycles.iter().any(|cycle| cycle.iter().any(|u| u == uri)) {
            continue;
        }
        let mut path = vec![uri.clone()];
        if let Some(cycle) = do_find_cycle(uri, imports, &mut path) {
            cycles.push(cycle);
        }
    }
    cycles
}

fn do_find_cycle(
    current_uri: &str,
    imports: &BTreeMap<String, Vec<String>>,
    path: &mut Vec<String>,
) -> Option<Vec<String>> {
    let next_uris = imports.get(current_uri)?;
    let starting_uri = path[0].clone();
    for uri in next_uris {
        if uri == &starting_uri {
            return Some(path.clone());
        }
        if path.iter().any(|p| p == uri) {
            // There is a cycle, but it does not start at `starting_uri`.
            return None;
        }
        path.push(uri.clone());
        if let Some(cycle) = do_find_cycle(uri, imports, path) {
            return Some(cycle);
        }
        path.pop();
    }
    None
}

/// Filters cycles to those containing at least one URI with the same scheme as
/// `root_project_uri` (case-insensitive), matching `Project.findImportCycle` (`Project.java:228-235`).
pub fn filter_same_scheme_cycles(
    root_project_uri: &str,
    cycles: Vec<Vec<String>>,
) -> Vec<Vec<String>> {
    let Some(root_scheme) = uri_scheme(root_project_uri) else {
        return cycles;
    };
    cycles
        .into_iter()
        .filter(|cycle| {
            cycle
                .iter()
                .any(|uri| uri_scheme(uri).is_some_and(|s| s.eq_ignore_ascii_case(root_scheme)))
        })
        .collect()
}

fn uri_scheme(uri: &str) -> Option<&str> {
    let idx = uri.find(':')?;
    if idx == 0 { None } else { Some(&uri[..idx]) }
}

/// Renders a single box-drawing cycle diagram matching `Project.renderCycle` (`Project.java:192-205`).
pub fn render_cycle(cycle: &[String]) -> String {
    let mut out = String::new();
    render_cycle_into(&mut out, cycle);
    out
}

fn render_cycle_into(out: &mut String, cycle: &[String]) {
    out.push_str("┌─>");
    for (idx, uri) in cycle.iter().enumerate() {
        if idx > 0 {
            out.push_str("\n│");
        }
        out.push_str("\n│  ");
        out.push_str(uri);
    }
    out.push_str("\n└─");
}

/// Renders multiple numbered cycles matching `Project.renderMultipleCycles` (`Project.java:177-190`).
pub fn render_multiple_cycles(cycles: &[Vec<String>]) -> String {
    let mut out = String::new();
    for (idx, cycle) in cycles.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        use std::fmt::Write;
        let _ = writeln!(&mut out, "Cycle {}:", idx + 1);
        render_cycle_into(&mut out, cycle);
        out.push('\n');
    }
    out
}

/// Formats the complete Pkl 0.32 circular local-project dependency error message
/// (`cannotHaveCircularProjectDependenciesSingle` / `Multiple`).
pub fn format_circular_project_dependencies_error(cycles: &[Vec<String>]) -> Option<String> {
    match cycles.len() {
        0 => None,
        1 => Some(format!(
            "Local project dependencies cannot be circular.\n\nCycle:\n{}",
            render_cycle(&cycles[0])
        )),
        _ => Some(format!(
            "Local project dependencies cannot be circular.\n\nThe following circular imports were found.\nNot all of them are necessarily problematic.\nThe problematic cycles are those declared as local dependencies.\n\n{}",
            render_multiple_cycles(cycles)
        )),
    }
}
