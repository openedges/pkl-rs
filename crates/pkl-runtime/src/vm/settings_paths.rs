// Copyright © 2026 Apple Inc. and the Pkl project authors. All rights reserved.
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at http://www.apache.org/licenses/LICENSE-2.0
// Unless required by applicable law or agreed to in writing, software distributed
// under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR
// CONDITIONS OF ANY KIND, either express or implied. See the License for the
// specific language governing permissions and limitations under the License.
//
// Source: pinned Pkl 0.32.0 util/PathResolver.java and EvaluatorSettingsNodes.java.
// Cross-OS path resolution is lexical and never consults the executing Host OS.
use super::*;

impl Vm {
    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn resolve_evaluator_settings_path(&mut self) -> Result<(), RuntimeError> {
        let locals = &self
            .frames
            .last()
            .ok_or_else(|| invalid("settings call frame"))?
            .environment
            .locals;
        let start = locals
            .len()
            .checked_sub(3)
            .ok_or_else(|| invalid("settings arguments"))?;
        let args = locals
            .get(start..)
            .ok_or_else(|| invalid("settings arguments"))?;
        let [
            Value::String(base),
            Value::String(path),
            Value::Bool(windows),
        ] = args
        else {
            return Err(invalid("typed settings path arguments"));
        };
        let result = resolve_path(base, path, *windows)?;
        self.stack.push(Value::String(result));
        Ok(())
    }
}

fn drive(path: &[u16]) -> bool {
    path.len() >= 2 && matches!(path[0], 65..=90 | 97..=122) && path[1] == 58
}
fn starts(path: &[u16], prefix: &[u16]) -> bool {
    path.starts_with(prefix)
}
fn windows_root(path: &[u16]) -> Vec<u16> {
    if starts(path, &[92, 92]) {
        let Some(first) = path[2..].iter().position(|u| *u == 92).map(|i| i + 2) else {
            return path.to_vec();
        };
        return match path[first + 1..]
            .iter()
            .position(|u| *u == 92)
            .map(|i| i + first + 1)
        {
            Some(second) => path[..=second].to_vec(),
            None => {
                let mut root = path.to_vec();
                root.push(92);
                root
            }
        };
    }
    if drive(path) {
        return path[..if path.get(2) == Some(&92) { 3 } else { 2 }].to_vec();
    }
    if path.first() == Some(&92) {
        return vec![92];
    }
    Vec::new()
}
fn normalize(path: &[u16], windows: bool) -> Result<PklString, RuntimeError> {
    let separator = if windows { 92 } else { 47 };
    let path: Vec<u16> = path
        .iter()
        .map(|unit| if windows && *unit == 47 { 92 } else { *unit })
        .collect();
    let root = if windows {
        windows_root(&path)
    } else {
        vec![47]
    };
    let Some(remainder) = path.get(root.len()..) else {
        // The pinned Java getRoot can append a separator to an incomplete UNC
        // share, after which substring throws an internal exception. Do not
        // fabricate an ordinary normalized value or a JVM exception trace.
        return Err(RuntimeError::new(
            ErrorKind::Unsupported,
            "Incomplete UNC root requires Java internal exception projection.",
        ));
    };
    let mut stack: Vec<&[u16]> = Vec::new();
    for part in remainder.split(|unit| *unit == separator) {
        if part == [46, 46] {
            stack.pop();
        } else if !part.is_empty() && part != [46] {
            stack.push(part);
        }
    }
    let mut result = root;
    for (index, part) in stack.into_iter().enumerate() {
        if index != 0 {
            result.push(separator);
        }
        result.extend_from_slice(part);
    }
    // Java String.split(regex) drops trailing empty components. Preserve that
    // observable rule rather than the source comment's trailing-slash intent.
    Ok(PklString::from_utf16(result))
}
fn resolve_path(
    base: &PklString,
    path: &PklString,
    windows: bool,
) -> Result<PklString, RuntimeError> {
    let (directory, host) = crate::host::resources::evaluator_settings_directory(base)?;
    let path: Vec<u16> = path
        .encode_utf16()
        .map(|u| if windows && u == 47 { 92 } else { u })
        .collect();
    let absolute = if windows {
        drive(&path) || starts(&path, &[92, 92])
    } else {
        path.first() == Some(&47)
    };
    if absolute {
        return normalize(&path, windows);
    }
    let mut base: Vec<u16> = directory.encode_utf16().collect();
    if windows {
        if let Some(host) = host {
            let mut unc = vec![92, 92];
            unc.extend(host.encode_utf16());
            unc.extend(base);
            base = unc;
        } else if base.len() >= 4
            && base[0] == 47
            && matches!(base[1], 65..=90)
            && base[2] == 58
            && base[3] == 47
            && !base[4..]
                .iter()
                .any(|u| matches!(*u, 10 | 13 | 133 | 8232 | 8233))
        {
            base.remove(0);
        }
        for unit in &mut base {
            if *unit == 47 {
                *unit = 92;
            }
        }
    }
    let separator = if windows { 92 } else { 47 };
    let mut joined = if windows && path.first() == Some(&92) {
        windows_root(&base)
    } else {
        base
    };
    if !(windows && path.first() == Some(&92)) && joined.last() != Some(&separator) {
        joined.push(separator);
    }
    joined.extend(path);
    normalize(&joined, windows)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn check(base: &str, path: &str, windows: bool, expected: &str) {
        assert_eq!(
            resolve_path(&base.into(), &path.into(), windows).unwrap(),
            PklString::from(expected)
        );
    }
    #[test]
    fn posix_directory_resolution_decodes_once_and_clamps_parent_segments() {
        check(
            "file:///path/to/dir/PklProject",
            "../x/./y",
            false,
            "/path/to/x/y",
        );
        check(
            "file:///path/space%20and%2520/PklProject",
            "x",
            false,
            "/path/space and%20/x",
        );
        check("file:/path/to/PklProject", "../../../../", false, "/");
    }
    #[test]
    fn windows_resolution_uses_requested_os_not_the_running_host() {
        check(
            "file:///C:/path/to/dir/PklProject",
            r"..\x\y",
            true,
            r"C:\path\to\x\y",
        );
        check(
            "file:///C:/path/to/dir/PklProject",
            r"\foo\bar",
            true,
            r"C:\foo\bar",
        );
        check("file:///C:/path/PklProject", "D:relative/../x", true, "D:x");
        check("file:///c:/path/PklProject", "x", true, r"\c:\path\x");
    }
    #[test]
    fn unc_host_spelling_and_share_root_are_preserved() {
        check(
            "file://Server/share/dir/PklProject",
            "../x",
            true,
            r"\\Server\share\x",
        );
        check(
            "file:///C:/path/PklProject",
            r"\\server\share\foo\..\bar",
            true,
            r"\\server\share\bar",
        );
    }
    #[test]
    fn trailing_separators_follow_java_split_and_drive_roots_remain() {
        check("file:///path/dir/PklProject", "x/", false, "/path/dir/x");
        check("file:///C:/path/PklProject", "C:/", true, "C:\\");
    }
    #[test]
    fn path_values_preserve_non_scalar_utf16_units() {
        let value = PklString::from_utf16(vec![120, 0xd800, 47, 121]);
        assert_eq!(
            resolve_path(&"file:///tmp/PklProject".into(), &value, false)
                .unwrap()
                .encode_utf16()
                .collect::<Vec<_>>(),
            vec![47, 116, 109, 112, 47, 120, 0xd800, 47, 121]
        );
    }
}
