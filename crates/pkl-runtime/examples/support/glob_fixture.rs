//! Finite authored fixture responses for the native portable glob driver.
//! This module does not match globs, resolve URIs, evaluate Pkl or synthesize Java
//! callbacks. Decisions and actual typed Host effects are separate channels.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use pkl_runtime::glob::PathElement;
use pkl_runtime::source::SourceText;
use pkl_runtime::value::PklString;

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug)]
pub struct Rule {
    pub operation: String,
    pub uri: PklString,
    pub attempt: u64,
    pub kind: String,
    pub value: PklString,
}
#[derive(Clone, Debug)]
pub struct OwnerSetting {
    pub prefix: PklString,
    pub suffix: PklString,
    pub key: PklString,
    pub value: PklString,
}
#[derive(Clone, Debug)]
pub struct Query {
    pub id: String,
    pub expression: SourceText,
}
pub struct Fixture {
    pub config: BTreeMap<String, PklString>,
    pub listings: BTreeMap<PklString, Vec<PathElement>>,
    pub modules: BTreeMap<PklString, SourceText>,
    pub resources: BTreeMap<PklString, PklString>,
    pub present: BTreeSet<PklString>,
    pub rules: Vec<Rule>,
    pub owners: Vec<OwnerSetting>,
    pub queries: Vec<Query>,
    pub source: SourceText,
}

pub fn unhex(text: &str) -> Result<PklString> {
    if !text.len().is_multiple_of(4) || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("invalid UTF-16 fixture hex".into());
    }
    let units: Vec<u16> = text
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|chunk| {
            // Every byte was admitted as ASCII hex above. No source string is decoded.
            u16::from_str_radix(std::str::from_utf8(chunk).unwrap(), 16).unwrap()
        })
        .collect();
    Ok(PklString::from_utf16(units))
}
fn table(directory: &Path, name: &str, width: usize) -> Result<Vec<Vec<String>>> {
    let bytes = std::fs::read(directory.join(name)).map_err(|error| format!("{name}: {error}"))?;
    if !bytes.is_ascii() || (!bytes.is_empty() && !bytes.ends_with(b"\n")) {
        return Err(format!("non-ASCII or unterminated fixture {name}"));
    }
    let text = String::from_utf8(bytes).unwrap();
    if text.is_empty() {
        return Ok(Vec::new());
    }
    text[..text.len() - 1]
        .split('\n')
        .map(|line| {
            let fields: Vec<_> = line.split('\t').map(str::to_owned).collect();
            if fields.len() != width {
                return Err(format!("invalid fixture width {name}"));
            }
            Ok(fields)
        })
        .collect()
}
fn flag(value: &PklString) -> Result<bool> {
    match value.as_str() {
        Some("true") => Ok(true),
        Some("false") => Ok(false),
        _ => Err("invalid fixture boolean".into()),
    }
}
fn pairs(directory: &Path, name: &str) -> Result<BTreeMap<PklString, PklString>> {
    let mut output = BTreeMap::new();
    for row in table(directory, name, 2)? {
        // Match the fixture Map construction: a later identical key replaces it.
        output.insert(unhex(&row[0])?, unhex(&row[1])?);
    }
    Ok(output)
}
impl Fixture {
    pub fn read(directory: &Path) -> Result<Self> {
        let mut config = BTreeMap::new();
        for row in table(directory, "config.tsv", 2)? {
            if config.insert(row[0].clone(), unhex(&row[1])?).is_some() {
                return Err("duplicate fixture config".into());
            }
        }
        let mut listings = BTreeMap::<PklString, Vec<PathElement>>::new();
        for row in table(directory, "listings.tsv", 3)? {
            listings
                .entry(unhex(&row[0])?)
                .or_default()
                .push(PathElement {
                    name: unhex(&row[1])?,
                    is_directory: flag(&row[2].as_str().into())?,
                });
        }
        for row in table(directory, "empty-lists.tsv", 1)? {
            listings.entry(unhex(&row[0])?).or_default();
        }
        let mut rules = Vec::new();
        for row in table(directory, "rules.tsv", 5)? {
            if row[2].starts_with('0')
                || row[2].is_empty()
                || !row[2].bytes().all(|b| b.is_ascii_digit())
            {
                return Err("invalid fixture attempt".into());
            }
            let attempt: u64 = row[2].parse().map_err(|_| "invalid fixture attempt")?;
            if attempt > (1_u64 << 53) - 1 {
                return Err("fixture attempt exceeds shared safe integer".into());
            }
            rules.push(Rule {
                operation: row[0].clone(),
                uri: unhex(&row[1])?,
                attempt,
                kind: row[3].clone(),
                value: unhex(&row[4])?,
            });
        }
        let mut owners = Vec::new();
        for row in table(directory, "owner-config.tsv", 4)? {
            owners.push(OwnerSetting {
                prefix: unhex(&row[0])?,
                suffix: unhex(&row[1])?,
                key: unhex(&row[2])?,
                value: unhex(&row[3])?,
            });
        }
        let mut queries = Vec::new();
        for row in table(directory, "queries.tsv", 2)? {
            queries.push(Query {
                id: row[0].clone(),
                expression: SourceText::from(unhex(&row[1])?),
            });
        }
        let source = std::fs::read_to_string(directory.join("module.pkl"))
            .map_err(|error| format!("module.pkl: {error}"))?
            .into();
        Ok(Self {
            config,
            listings,
            modules: pairs(directory, "modules.tsv")?
                .into_iter()
                .map(|(uri, text)| (uri, text.into()))
                .collect(),
            resources: pairs(directory, "resources.tsv")?,
            present: table(directory, "present.tsv", 1)?
                .into_iter()
                .map(|row| unhex(&row[0]))
                .collect::<Result<_>>()?,
            rules,
            owners,
            queries,
            source,
        })
    }
}

#[derive(Clone, Debug)]
pub struct Reader {
    pub owner: PklString,
    pub scheme: PklString,
    pub selected_uri: PklString,
    #[allow(
        dead_code,
        reason = "Provider identity is retained in Debug traces and asserted by fixture tests."
    )]
    pub provider_index: usize,
}
#[derive(Clone, Debug)]
#[allow(
    dead_code,
    reason = "These complete callback fields are emitted by derived Debug; some consumers only assert evaluator results."
)]
pub struct Decision {
    pub index: usize,
    pub phase: String,
    pub operation: String,
    pub owner: PklString,
    pub values: Vec<PklString>,
}
pub struct Provider {
    pub fixture: Fixture,
    pub phase: String,
    pub decisions: Vec<Decision>,
    attempts: BTreeMap<(String, PklString), u64>,
}
fn starts(value: &PklString, prefix: &PklString) -> bool {
    value
        .encode_utf16()
        .take(prefix.encode_utf16().count())
        .eq(prefix.encode_utf16())
}
fn ends(value: &PklString, suffix: &PklString) -> bool {
    value.encode_utf16().count() >= suffix.encode_utf16().count()
        && value
            .encode_utf16()
            .skip(value.encode_utf16().count() - suffix.encode_utf16().count())
            .eq(suffix.encode_utf16())
}
impl Provider {
    pub fn new(fixture: Fixture) -> Self {
        Self {
            fixture,
            phase: "bootstrap".into(),
            decisions: Vec::new(),
            attempts: BTreeMap::new(),
        }
    }
    pub fn decision(&mut self, operation: &str, owner: PklString, values: Vec<PklString>) {
        self.decisions.push(Decision {
            index: self.decisions.len(),
            phase: self.phase.clone(),
            operation: operation.into(),
            owner,
            values,
        });
    }
    pub fn setting(&self, owner: &PklString, key: &str) -> Option<&PklString> {
        let mut value = self.fixture.config.get(key);
        for setting in &self.fixture.owners {
            if starts(owner, &setting.prefix) && ends(owner, &setting.suffix) && setting.key == key
            {
                value = Some(&setting.value);
            }
        }
        value
    }
    pub fn boolean(&self, owner: &PklString, key: &str) -> Result<bool> {
        flag(
            self.setting(owner, key)
                .ok_or_else(|| format!("missing fixture setting {key}"))?,
        )
    }
    pub fn rule(&mut self, operation: &str, uri: &PklString) -> Result<Option<Rule>> {
        if matches!(
            operation,
            "resolve" | "load-source" | "read" | "fragment-paths" | "globbable" | "hierarchical"
        ) && ["sourceUri", "resolvedPrimary"]
            .iter()
            .any(|key| self.fixture.config.get(*key) == Some(uri))
        {
            return Ok(None);
        }
        let attempt = self
            .attempts
            .entry((operation.into(), uri.clone()))
            .or_default();
        *attempt = attempt.checked_add(1).ok_or("fixture attempt overflow")?;
        let attempt = *attempt;
        let mut selected: Option<Rule> = None;
        for rule in &self.fixture.rules {
            let units: Vec<_> = rule.uri.encode_utf16().collect();
            let wildcard_scheme = units.ends_with(&[u16::from(b':'), u16::from(b'*')])
                && starts(
                    uri,
                    &PklString::from_utf16(units[..units.len() - 1].to_vec()),
                );
            let matches = rule.uri == "*" || rule.uri == *uri || wildcard_scheme;
            if rule.operation == operation
                && matches
                && rule.attempt <= attempt
                && selected.as_ref().is_none_or(|old| {
                    rule.attempt > old.attempt || (rule.uri != "*" && old.uri == "*")
                })
            {
                selected = Some(rule.clone());
            }
        }
        if let Some(rule) = &selected {
            self.decision(
                "fixture.rule",
                operation.into(),
                vec![
                    uri.clone(),
                    attempt.to_string().into(),
                    rule.kind.clone().into(),
                    rule.value.clone(),
                ],
            );
        }
        Ok(selected)
    }
    pub fn resource_reader(&self, scheme: &PklString) -> Result<Option<Reader>> {
        let configured = self
            .fixture
            .config
            .get("resourceSchemes")
            .and_then(PklString::as_str)
            .ok_or("missing/non-scalar resourceSchemes")?;
        let mut selected = None;
        for (index, candidate) in configured
            .split(',')
            .filter(|part| !part.is_empty())
            .enumerate()
        {
            if scheme == candidate {
                selected = Some(Reader {
                    owner: format!("resource:{candidate}:{index}").into(),
                    scheme: candidate.into(),
                    selected_uri: format!("{candidate}:configured").into(),
                    provider_index: index,
                });
            }
        }
        Ok(selected)
    }
    pub fn module_reader(&self, uri: &PklString) -> Result<Option<Reader>> {
        let units: Vec<_> = uri.encode_utf16().collect();
        let Some(colon) = units.iter().position(|unit| *unit == u16::from(b':')) else {
            return Ok(None);
        };
        let scheme = PklString::from_utf16(units[..colon].to_vec());
        let configured = self
            .fixture
            .config
            .get("moduleProviderSchemes")
            .or_else(|| self.fixture.config.get("moduleSchemes"))
            .and_then(PklString::as_str)
            .ok_or("missing/non-scalar module provider schemes")?;
        let Some(index) = configured
            .split(';')
            .position(|group| group.split(',').any(|candidate| scheme == candidate))
        else {
            return Ok(None);
        };
        Ok(Some(Reader {
            owner: PklString::from("module:")
                .concat(uri)
                .concat(&format!("@{index}").into()),
            scheme,
            selected_uri: uri.clone(),
            provider_index: index,
        }))
    }
    /// Lookup only, after the Host has applied the script and security policy.
    /// Keeping this separate prevents an unconfigured-table error from masking
    /// the Java helper's earlier security rejection.
    pub fn elements_after_admission(
        &self,
        reader: &Reader,
        uri: &PklString,
        rule: Option<&Rule>,
    ) -> Result<Vec<PathElement>> {
        let key = if let Some(rule) = rule.filter(|rule| rule.kind == "elements") {
            PklString::from("script:").concat(&rule.value)
        } else {
            let base = if self.boolean(&reader.owner, "hierarchical")? {
                uri.clone()
            } else {
                "*".into()
            };
            self.setting(&reader.owner, "listingPrefix")
                .cloned()
                .unwrap_or_default()
                .concat(&base)
        };
        self.fixture
            .listings
            .get(&key)
            .cloned()
            .ok_or_else(|| "unconfigured listing (raw key retained in typed request trace)".into())
    }
    pub fn exists(&mut self, uri: &PklString) -> Result<(Option<Rule>, bool)> {
        Ok((
            self.rule("has-element", uri)?,
            self.fixture.present.contains(uri),
        ))
    }
    pub fn resource(&mut self, uri: &PklString) -> Result<std::result::Result<PklString, Rule>> {
        if let Some(rule) = self.rule("read", uri)? {
            return Ok(Err(rule));
        }
        self.fixture
            .resources
            .get(uri)
            .cloned()
            .map(Ok)
            .ok_or_else(|| "unconfigured resource (raw URI retained in typed request trace)".into())
    }
    pub fn module_source(
        &mut self,
        uri: &PklString,
    ) -> Result<std::result::Result<SourceText, Rule>> {
        if let Some(rule) = self.rule("load-source", uri)? {
            return Ok(Err(rule));
        }
        self.fixture
            .modules
            .get(uri)
            .cloned()
            .map(Ok)
            .ok_or_else(|| {
                "unconfigured module source (raw URI retained in typed request trace)".into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn empty_fixture() -> Fixture {
        Fixture {
            config: BTreeMap::new(),
            listings: BTreeMap::new(),
            modules: BTreeMap::new(),
            resources: BTreeMap::new(),
            present: BTreeSet::new(),
            rules: Vec::new(),
            owners: Vec::new(),
            queries: Vec::new(),
            source: "".into(),
        }
    }
    #[test]
    fn fixture_hex_preserves_lone_units_pairs_and_scalar_distinctions() {
        let raw = unhex("0067d8000078dfff").unwrap();
        assert_eq!(
            raw.encode_utf16().collect::<Vec<_>>(),
            [0x67, 0xd800, 0x78, 0xdfff]
        );
        assert_ne!(unhex("d800").unwrap(), unhex("e000").unwrap());
        assert_ne!(unhex("d800").unwrap(), unhex("fffd").unwrap());
        assert_eq!(unhex("d83dde00").unwrap(), "😀");
        assert!(unhex("d8000").is_err());
        assert!(unhex("d80g").is_err());
        let source = SourceText::from(raw.clone());
        assert_eq!(source.original(), &raw);
    }
    #[test]
    fn duplicate_resource_and_module_profiles_keep_their_distinct_registration_order() {
        let mut fixture = empty_fixture();
        fixture
            .config
            .insert("resourceSchemes".into(), "gdata,gdata".into());
        fixture
            .config
            .insert("moduleProviderSchemes".into(), "gmod;gmod".into());
        let provider = Provider::new(fixture);
        assert_eq!(
            provider
                .resource_reader(&"gdata".into())
                .unwrap()
                .unwrap()
                .owner,
            "resource:gdata:1"
        );
        let uri = PklString::from("gmod:").concat(&PklString::from_utf16(vec![0xd800]));
        let selected = provider.module_reader(&uri).unwrap().unwrap();
        assert_eq!(selected.provider_index, 0);
        assert_eq!(selected.selected_uri, uri);
        assert_eq!(
            selected.owner.encode_utf16().collect::<Vec<_>>(),
            "module:gmod:"
                .encode_utf16()
                .chain([0xd800])
                .chain("@0".encode_utf16())
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn primary_exclusion_is_exact_identity_not_scheme_and_retries_select_later_script() {
        let mut fixture = empty_fixture();
        fixture
            .config
            .insert("sourceUri".into(), "gmain:/suite.pkl".into());
        fixture
            .config
            .insert("resolvedPrimary".into(), "gmain:/primary.pkl".into());
        fixture.rules = vec![
            Rule {
                operation: "read".into(),
                uri: "gmain:*".into(),
                attempt: 1,
                kind: "io".into(),
                value: "first failure".into(),
            },
            Rule {
                operation: "read".into(),
                uri: "gmain:/value".into(),
                attempt: 2,
                kind: "value".into(),
                value: PklString::from_utf16(vec![0xdfff]),
            },
        ];
        let mut provider = Provider::new(fixture);
        assert!(
            provider
                .rule("read", &"gmain:/suite.pkl".into())
                .unwrap()
                .is_none()
        );
        assert!(
            provider
                .rule("read", &"gmain:/primary.pkl".into())
                .unwrap()
                .is_none()
        );
        assert_eq!(
            provider
                .rule("read", &"gmain:/value".into())
                .unwrap()
                .unwrap()
                .kind,
            "io"
        );
        let second = provider
            .rule("read", &"gmain:/value".into())
            .unwrap()
            .unwrap();
        assert_eq!(second.kind, "value");
        assert_eq!(second.value.encode_utf16().collect::<Vec<_>>(), [0xdfff]);
        assert_eq!(provider.decisions.len(), 2);
    }
}
