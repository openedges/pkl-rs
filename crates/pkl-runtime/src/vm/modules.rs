use super::*;
use crate::modules::{
    ModuleCompletion, ModuleIdentity, ModuleKey, ModuleKeyId, ModuleKeyKind, ModuleRequest,
    ModuleRequestKind, ModuleSource, ResolvedKeyId, ResolvedModule, ResolvedModuleKey,
    SelectionPhase, UnitId,
};
use crate::source::SourceText;
use crate::value::RuntimeSource;

mod inheritance;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModuleMode {
    Plain,
    Extends,
    Amends,
}

#[derive(Clone, Debug)]
pub(crate) struct ModuleUnit {
    pub id: UnitId,
    pub identity: Option<ModuleIdentity>,
    pub standard_library: bool,
    pub singleton_cache_key: Option<&'static str>,
    pub key: Option<ModuleKey>,
    pub delegate_key: Option<ModuleKey>,
    pub build: Option<ModuleBuild>,
    pub syntax: Option<Arc<crate::syntax::Module>>,
    pub source: Option<Arc<RuntimeSource>>,
    pub name: PklString,
    pub template: usize,
    pub entry: usize,
    pub blocks_start: usize,
    pub initial_members: Vec<Member>,
    pub(super) import_order: Vec<uri_resolution::PreparationStep>,
    pub class_id: usize,
    pub mode: ModuleMode,
    pub modifiers: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct ModuleBuild {
    pub module: Arc<crate::syntax::Module>,
    pub resolved: Option<Arc<crate::resolver::ResolvedModule>>,
    pub name_changed: bool,
    pub options: CompileOptions,
    pub instrument: bool,
    pub prepared: BTreeMap<Span, (PklString, PklString)>,
}

impl ModuleUnit {
    pub(super) fn singleton_cache_key(&self) -> Option<&'static str> {
        self.singleton_cache_key.or_else(|| {
            if !self.standard_library {
                return None;
            }
            self.identity.as_ref().and_then(|identity| {
                match crate::modules::stdlib_kind(&identity.original_uri) {
                    Some(crate::modules::StdlibKind::Singleton(canonical)) => Some(canonical),
                    _ => None,
                }
            })
        })
    }
}

#[derive(Clone, Debug)]
pub struct ImportSpec {
    pub authored: PklString,
    pub span: Span,
    pub unit: UnitId,
    pub absolute_uri: PklString,
    pub original_uri: PklString,
}
impl ImportSpec {
    pub(super) fn new(
        authored: PklString,
        span: Span,
        unit: UnitId,
        identity: Option<&ModuleIdentity>,
    ) -> Result<Self, RuntimeError> {
        let (absolute, original) = crate::host::resources::prepare_module_uri(
            &authored,
            identity.map(|m| &m.original_uri),
        )
        .map_err(|e| e.at(span))?;
        Ok(Self {
            authored,
            span,
            unit,
            absolute_uri: absolute,
            original_uri: original,
        })
    }
    pub(super) fn authored(authored: PklString, span: Span, unit: UnitId) -> Self {
        Self {
            absolute_uri: authored.clone(),
            original_uri: authored.clone(),
            authored,
            span,
            unit,
        }
    }
}

impl Program {
    /// Register the primary ModuleKey separately from its display provenance.
    /// This prepares all import URI operands before any guest member is forced.
    pub fn register_primary_source(
        &mut self,
        source: Arc<RuntimeSource>,
        identity: ModuleIdentity,
    ) -> Result<(), RuntimeError> {
        let Some(unit) = self.units.get_mut(0) else {
            return Ok(());
        };
        unit.source = Some(source.clone());
        unit.name = source.module_name.clone();
        unit.identity = Some(identity.clone());
        self.module_name = source.module_name.clone();
        Ok(())
    }
}

#[derive(Clone)]
pub(super) enum CachedModule {
    Value(ObjectId),
    Failed(RuntimeError),
}

#[inline(always)]
fn is_clean_cache_uri(uri: &PklString) -> bool {
    uri.as_str().is_some_and(|s| {
        crate::host::resources::is_clean_file_uri(s)
            || (s.starts_with("pkl:")
                && s.is_ascii()
                && !s.bytes().any(|b| matches!(b, b'%' | b'?' | b'#' | b'\\')))
    })
}

/// Module URI equality shares the Java URI.equals rules already used by resources.
/// For small sets of clean `file:///` and `pkl:` URIs (already canonical ASCII),
/// stores entries inline in a flat `Vec` with zero `BTreeMap` or `CacheKey` overhead.
#[derive(Clone)]
pub(super) struct ModuleCache {
    small: Vec<(PklString, CachedModule)>,
    all_clean: bool,
    entries: BTreeMap<crate::host::resources::CacheKey, CachedModule>,
    exact: BTreeMap<PklString, CachedModule>,
    last: Option<(PklString, CachedModule)>,
}
impl Default for ModuleCache {
    fn default() -> Self {
        Self {
            small: Vec::new(),
            all_clean: true,
            entries: BTreeMap::new(),
            exact: BTreeMap::new(),
            last: None,
        }
    }
}
impl ModuleCache {
    fn promote_to_maps(&mut self) {
        if !self.small.is_empty() {
            for (u, v) in self.small.drain(..) {
                let key = crate::host::resources::cache_key(&u);
                self.entries.insert(key, v.clone());
                self.exact.insert(u, v);
            }
        }
        self.all_clean = false;
    }

    #[inline]
    pub fn get(&self, uri: impl Into<PklString>) -> Option<&CachedModule> {
        let uri = uri.into();
        self.get_ref(&uri)
    }
    #[inline]
    pub fn get_ref(&self, uri: &PklString) -> Option<&CachedModule> {
        if self.all_clean {
            for (u, v) in self.small.iter().rev() {
                if u == uri {
                    return Some(v);
                }
            }
            if is_clean_cache_uri(uri) {
                return None;
            }
            let target = crate::host::resources::cache_key(uri);
            for (u, v) in &self.small {
                if crate::host::resources::cache_key(u) == target {
                    return Some(v);
                }
            }
            return None;
        }
        if let Some((last_uri, last_val)) = &self.last
            && last_uri == uri
        {
            return Some(last_val);
        }
        if let Some(v) = self.exact.get(uri) {
            return Some(v);
        }
        self.entries.get(&crate::host::resources::cache_key(uri))
    }
    pub fn insert(&mut self, uri: PklString, value: CachedModule) {
        if self.all_clean {
            if is_clean_cache_uri(&uri) {
                for (u, v) in &mut self.small {
                    if *u == uri {
                        *v = value;
                        return;
                    }
                }
                if self.small.len() < 8 {
                    self.small.push((uri, value));
                    return;
                }
            }
            self.promote_to_maps();
        }
        let key = crate::host::resources::cache_key(&uri);
        self.entries.insert(key, value.clone());
        self.exact.insert(uri.clone(), value.clone());
        self.last = Some((uri, value));
    }
    pub fn remove(&mut self, uri: impl Into<PklString>) -> Option<CachedModule> {
        let uri = uri.into();
        if self.all_clean {
            if let Some(pos) = self.small.iter().position(|(u, _)| *u == uri) {
                return Some(self.small.swap_remove(pos).1);
            }
            if is_clean_cache_uri(&uri) {
                return None;
            }
            let target = crate::host::resources::cache_key(&uri);
            if let Some(pos) = self
                .small
                .iter()
                .position(|(u, _)| crate::host::resources::cache_key(u) == target)
            {
                return Some(self.small.swap_remove(pos).1);
            }
            return None;
        }
        let key = crate::host::resources::cache_key(&uri);
        self.exact
            .retain(|k, _| crate::host::resources::cache_key(k) != key);
        if let Some((last_uri, _)) = &self.last
            && crate::host::resources::cache_key(last_uri) == key
        {
            self.last = None;
        }
        self.entries.remove(&key)
    }
    pub fn clear(&mut self) {
        self.small.clear();
        self.all_clean = true;
        self.entries.clear();
        self.exact.clear();
        self.last = None;
    }
    pub fn values(&self) -> impl Iterator<Item = &CachedModule> {
        self.small
            .iter()
            .map(|(_, v)| v)
            .chain(self.entries.values())
    }
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut CachedModule> {
        self.exact.clear();
        self.last = None;
        self.small
            .iter_mut()
            .map(|(_, v)| v)
            .chain(self.entries.values_mut())
    }
}

static NEXT_KEY: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
pub(super) fn next_key_id() -> Result<u64, RuntimeError> {
    NEXT_KEY
        .fetch_update(
            std::sync::atomic::Ordering::Relaxed,
            std::sync::atomic::Ordering::Relaxed,
            |n| n.checked_add(1),
        )
        .map_err(|_| invalid("ModuleKey identity exhausted"))
}

pub(super) fn next_failure_id() -> u64 {
    next_key_id().expect("cached exception identity exhausted")
}

enum ParsedOrResolved {
    Parsed(Box<crate::syntax::Module>),
    Resolved(
        Arc<crate::resolver::ResolvedModule>,
        Arc<crate::syntax::Module>,
    ),
}

impl ParsedOrResolved {
    fn module(&self) -> &crate::syntax::Module {
        match self {
            Self::Parsed(module) => module,
            Self::Resolved(resolved, _) => &resolved.module,
        }
    }
}

type ParsedModule =
    Result<ParsedOrResolved, Box<(crate::parser::ModuleParseFailure, Option<PklString>)>>;

struct CachedFrontendEntry {
    hash: u64,
    text: SourceText,
    resolved: Arc<crate::resolver::ResolvedModule>,
    syntax: Arc<crate::syntax::Module>,
}

pub(super) fn source_text_hash(text: &SourceText) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    if let Some(s) = text.as_str() {
        for &b in s.as_bytes() {
            hash ^= u64::from(b);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    } else {
        for u in text.original().encode_utf16() {
            hash ^= u64::from(u);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    hash
}

fn frontend_cache() -> &'static std::sync::Mutex<Vec<CachedFrontendEntry>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<CachedFrontendEntry>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(Vec::with_capacity(32)))
}

fn cached_user_resolved(
    text: &SourceText,
) -> Option<(
    Arc<crate::resolver::ResolvedModule>,
    Arc<crate::syntax::Module>,
)> {
    // Only cache reasonably sized modules (avoid pinning huge generated text).
    if text.len_bytes() > 256 * 1024 {
        return None;
    }
    let hash = source_text_hash(text);
    let cache = frontend_cache().lock().ok()?;
    cache
        .iter()
        .rev()
        .find(|entry| entry.hash == hash && entry.text == *text)
        .map(|entry| (Arc::clone(&entry.resolved), Arc::clone(&entry.syntax)))
}

fn store_user_resolved(
    text: &SourceText,
    resolved: Arc<crate::resolver::ResolvedModule>,
    syntax: Arc<crate::syntax::Module>,
) {
    if text.len_bytes() > 256 * 1024 {
        return;
    }
    let hash = source_text_hash(text);
    if let Ok(mut cache) = frontend_cache().lock() {
        if cache
            .iter()
            .any(|entry| entry.hash == hash && entry.text == *text)
        {
            return;
        }
        if cache.len() >= 32 {
            cache.remove(0);
        }
        cache.push(CachedFrontendEntry {
            hash,
            text: text.clone(),
            resolved,
            syntax,
        });
    }
}

type ConstraintOriginsVec = Vec<(usize, (Vec<RuntimeFrame>, Vec<RuntimeFrame>))>;

struct CachedCompiledUnit {
    hash: u64,
    text: SourceText,
    source_uri: PklString,
    module_name: PklString,
    has_identity: bool,
    original_uri: PklString,
    resolved_uri: PklString,
    power_assertions: bool,
    fuse_constant_add: bool,
    pure_standard_host: bool,
    is_primary_root: bool,
    is_replacement: bool,
    prepared: Vec<(Span, (PklString, PklString))>,
    blocks_start: usize,
    objects_start: usize,
    recipes_start: usize,
    unit_id: u32,
    declaration_base: usize,
    base_mixin_declaration: Option<usize>,
    next_declaration_after: usize,
    blocks: Arc<Vec<Vec<Instruction>>>,
    metadata: Arc<Vec<BlockMetadata>>,
    objects: Arc<Vec<Vec<Member>>>,
    type_recipes: Arc<Vec<type_resolution::TypeRecipe>>,
    unit: ModuleUnit,
    declaration_sources: Arc<[(usize, crate::syntax::DeclarationSource)]>,
    reflection_annotations: Vec<((PklString, Span), usize)>,
    alias_definitions: Vec<(usize, type_resolution::AliasDefinition)>,
    constraint_origins: ConstraintOriginsVec,
}

const MAX_CACHED_UNIT_BLOCKS: usize = 8192;
const MAX_TOTAL_CACHED_UNIT_BLOCKS: usize = 32768;

fn compiled_unit_cache() -> &'static std::sync::Mutex<Vec<CachedCompiledUnit>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<CachedCompiledUnit>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(Vec::with_capacity(32)))
}

pub(super) struct CompiledUnitCacheKey<'a> {
    pub hash: u64,
    pub text: &'a SourceText,
    pub source_uri: &'a PklString,
    pub module_name: &'a PklString,
    pub has_identity: bool,
    pub original_uri: &'a PklString,
    pub resolved_uri: &'a PklString,
    pub power_assertions: bool,
    pub fuse_constant_add: bool,
    pub pure_standard_host: bool,
    pub is_primary_root: bool,
    pub is_replacement: bool,
    pub prepared: &'a [(Span, (PklString, PklString))],
    pub blocks_start: usize,
    pub objects_start: usize,
    pub recipes_start: usize,
    pub unit_id: u32,
    pub declaration_base: usize,
    pub base_mixin_declaration: Option<usize>,
}

pub(super) fn try_replay_compiled_unit(
    program: &mut Program,
    key: &CompiledUnitCacheKey<'_>,
) -> Result<Option<ModuleUnit>, RuntimeError> {
    let hit = {
        let Ok(cache) = compiled_unit_cache().lock() else {
            return Ok(None);
        };
        cache
            .iter()
            .rev()
            .find(|e| {
                e.hash == key.hash
                    && e.blocks_start == key.blocks_start
                    && e.objects_start == key.objects_start
                    && e.recipes_start == key.recipes_start
                    && e.unit_id == key.unit_id
                    && e.declaration_base == key.declaration_base
                    && e.base_mixin_declaration == key.base_mixin_declaration
                    && e.power_assertions == key.power_assertions
                    && e.fuse_constant_add == key.fuse_constant_add
                    && e.pure_standard_host == key.pure_standard_host
                    && e.is_primary_root == key.is_primary_root
                    && e.is_replacement == key.is_replacement
                    && e.has_identity == key.has_identity
                    && e.original_uri == *key.original_uri
                    && e.resolved_uri == *key.resolved_uri
                    && e.source_uri == *key.source_uri
                    && e.module_name == *key.module_name
                    && e.prepared == key.prepared
                    && e.text == *key.text
            })
            .map(|cached| {
                (
                    Arc::clone(&cached.blocks),
                    Arc::clone(&cached.metadata),
                    Arc::clone(&cached.objects),
                    Arc::clone(&cached.type_recipes),
                    cached.next_declaration_after,
                    cached.unit.clone(),
                    Arc::clone(&cached.declaration_sources),
                    cached.reflection_annotations.clone(),
                    cached.alias_definitions.clone(),
                    cached.constraint_origins.clone(),
                )
            })
    };
    let Some((
        blocks_seg,
        metadata_seg,
        objects_seg,
        recipes_seg,
        next_decl_after,
        unit_clone,
        decl_sources,
        refl_annots,
        alias_defs,
        constr_origins,
    )) = hit
    else {
        return Ok(None);
    };
    let blocks_end = key.blocks_start + blocks_seg.len();
    program.blocks.push_sealed_segment(blocks_seg)?;
    program.metadata.push_sealed_segment(metadata_seg)?;
    program.objects.push_sealed_segment(objects_seg)?;
    program.type_recipes.push_sealed_segment(recipes_seg)?;
    program.next_declaration = next_decl_after;
    if !decl_sources.is_empty() {
        program
            .extra_declaration_sources
            .push((key.blocks_start..blocks_end, decl_sources));
    }
    if !refl_annots.is_empty() {
        program.reflection_annotations.extend(refl_annots);
    }
    if !alias_defs.is_empty() {
        program.alias_definitions.extend(alias_defs);
    }
    if !constr_origins.is_empty() {
        program.constraint_origins.extend(constr_origins);
    }
    if key.is_primary_root {
        program.module_template = Some(unit_clone.template);
        program.module_name = unit_clone.name.clone();
        program.entry = unit_clone.entry;
        program.initial_module_members = unit_clone.initial_members.clone();
    }
    Ok(Some(unit_clone))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn store_compiled_unit(
    program: &Program,
    key: &CompiledUnitCacheKey<'_>,
    before_decl_len: usize,
    before_refl_len: usize,
    before_alias_len: usize,
    before_constr_len: usize,
    mut unit_snapshot: ModuleUnit,
) {
    let blocks_end = program.blocks.next_id();
    let objects_end = program.objects.next_id();
    let recipes_end = program.type_recipes.next_id();
    let new_blocks = blocks_end.saturating_sub(key.blocks_start);
    if new_blocks > MAX_CACHED_UNIT_BLOCKS || program.metadata.next_id() != blocks_end {
        return;
    }
    let decl_delta: Vec<_> = program
        .declaration_sources
        .range(key.blocks_start..blocks_end)
        .map(|(&k, v)| (k, v.clone()))
        .collect();
    let refl_delta: Vec<_> = program
        .reflection_annotations
        .iter()
        .filter(|(_, block)| **block >= key.blocks_start && **block < blocks_end)
        .map(|(k, &v)| (k.clone(), v))
        .collect();
    let alias_delta: Vec<_> = program
        .alias_definitions
        .range(key.declaration_base..program.next_declaration)
        .map(|(&k, v)| (k, v.clone()))
        .collect();
    let constr_delta: Vec<_> = program
        .constraint_origins
        .range(key.blocks_start..blocks_end)
        .map(|(&k, v)| (k, v.clone()))
        .collect();
    if program.declaration_sources.len() != before_decl_len + decl_delta.len()
        || program.reflection_annotations.len() != before_refl_len + refl_delta.len()
        || program.alias_definitions.len() != before_alias_len + alias_delta.len()
        || program.constraint_origins.len() != before_constr_len + constr_delta.len()
    {
        return;
    }
    let (Some(blocks_seg), Some(metadata_seg), Some(objects_seg), Some(recipes_seg)) = (
        program.blocks.slice_segment(key.blocks_start, blocks_end),
        program.metadata.slice_segment(key.blocks_start, blocks_end),
        program
            .objects
            .slice_segment(key.objects_start, objects_end),
        program
            .type_recipes
            .slice_segment(key.recipes_start, recipes_end),
    ) else {
        return;
    };
    unit_snapshot.key = None;
    unit_snapshot.delegate_key = None;
    if let Ok(mut cache) = compiled_unit_cache().lock() {
        let mut total_blocks: usize = cache.iter().map(|e| e.blocks.len()).sum();
        while !cache.is_empty()
            && (cache.len() >= 32 || total_blocks + new_blocks > MAX_TOTAL_CACHED_UNIT_BLOCKS)
        {
            let evicted = cache.remove(0);
            total_blocks = total_blocks.saturating_sub(evicted.blocks.len());
        }
        cache.push(CachedCompiledUnit {
            hash: key.hash,
            text: key.text.clone(),
            source_uri: key.source_uri.clone(),
            module_name: key.module_name.clone(),
            has_identity: key.has_identity,
            original_uri: key.original_uri.clone(),
            resolved_uri: key.resolved_uri.clone(),
            power_assertions: key.power_assertions,
            fuse_constant_add: key.fuse_constant_add,
            pure_standard_host: key.pure_standard_host,
            is_primary_root: key.is_primary_root,
            is_replacement: key.is_replacement,
            prepared: key.prepared.to_vec(),
            blocks_start: key.blocks_start,
            objects_start: key.objects_start,
            recipes_start: key.recipes_start,
            unit_id: key.unit_id,
            declaration_base: key.declaration_base,
            base_mixin_declaration: key.base_mixin_declaration,
            next_declaration_after: program.next_declaration,
            blocks: blocks_seg,
            metadata: metadata_seg,
            objects: objects_seg,
            type_recipes: recipes_seg,
            unit: unit_snapshot,
            declaration_sources: Arc::from(decl_delta),
            reflection_annotations: refl_delta,
            alias_definitions: alias_delta,
            constraint_origins: constr_delta,
        });
    }
}

struct CachedCompiledQuery {
    expr_hash: u64,
    expr_text: SourceText,
    module_text_hash: u64,
    module_text: SourceText,
    module_uri: PklString,
    module_name: PklString,
    raw_value: bool,
    power_assertions: bool,
    unit_id: u32,
    blocks_start: usize,
    objects_start: usize,
    recipes_start: usize,
    declaration_base: usize,
    next_declaration_after: usize,
    entry: usize,
    blocks: Arc<Vec<Vec<Instruction>>>,
    metadata: Arc<Vec<BlockMetadata>>,
    objects: Arc<Vec<Vec<Member>>>,
    type_recipes: Arc<Vec<type_resolution::TypeRecipe>>,
    declaration_sources: Arc<[(usize, crate::syntax::DeclarationSource)]>,
    reflection_annotations: Vec<((PklString, Span), usize)>,
    alias_definitions: Vec<(usize, type_resolution::AliasDefinition)>,
    constraint_origins: ConstraintOriginsVec,
}

fn compiled_query_cache() -> &'static std::sync::Mutex<Vec<CachedCompiledQuery>> {
    static CACHE: std::sync::OnceLock<std::sync::Mutex<Vec<CachedCompiledQuery>>> =
        std::sync::OnceLock::new();
    CACHE.get_or_init(|| std::sync::Mutex::new(Vec::with_capacity(16)))
}

pub(super) struct ModuleRun {
    pub spec: ImportSpec,
    site: Option<(ExecutableOwnerId, usize, usize)>,
    purpose: ModulePurpose,
    supplied: Option<SourceText>,
    internal_key: bool,
    phase: ModulePhase,
    key: Option<ModuleKey>,
    delegate_key: Option<ModuleKey>,
    resolved_key: Option<ResolvedModuleKey>,
    pub(super) cached: bool,
    pub resolved: Option<ResolvedModule>,
    pub receiver: Option<ObjectId>,
    pub initializing: bool,
    parsed: Option<ParsedModule>,
    source: Option<Arc<RuntimeSource>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModulePurpose {
    Import,
    Primary,
    Query,
    RawQuery,
}

impl ModuleRun {
    pub(super) fn is_import_initializing(&self) -> bool {
        self.initializing && self.purpose == ModulePurpose::Import
    }

    pub(super) fn is_singleton(&self) -> bool {
        self.key.as_ref().is_some_and(|key| {
            key.kind == ModuleKeyKind::StandardLibrary
                && matches!(
                    crate::modules::stdlib_kind(&key.uri),
                    Some(crate::modules::StdlibKind::Singleton(_))
                )
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ModulePhase {
    ImportCheck,
    SelectKey,
    CachePolicy,
    Resolve,
    Source,
    Name,
    Initializer,
}

impl Vm {
    pub(crate) fn append_public_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        expression: SourceText,
    ) -> Result<usize, RuntimeError> {
        crate::host::resources::validate_module_key_uri(&source.uri)?;
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let mut code = vec![Instruction::LoadPrimary(Arc::new(source))];
        if expression.original() == "output.text" {
            code.extend([
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("text".into()),
            ]);
        } else {
            code.push(Instruction::EvaluateQuery(expression));
        }
        code.push(Instruction::Return);
        let entry = program.alloc_block(code, BlockMetadata::default())?;
        program.seal_code_segment();
        self.public_operation_entries.insert(entry);
        Ok(entry)
    }

    /// Appends a public operation matching `EvaluatorImpl.evaluateExpressionPklBinary`:
    /// loads the primary module (using cache), evaluates `module` / `output.text` /
    /// `output.value` / `output.bytes` / arbitrary expression, and encodes the forced
    /// value into `pkl-binary` (`Value::Bytes`).
    pub(crate) fn append_public_pkl_binary_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
        expression: SourceText,
    ) -> Result<usize, RuntimeError> {
        crate::host::resources::validate_module_key_uri(&source.uri)?;
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let mut code = vec![Instruction::LoadPrimary(Arc::new(source))];
        match expression.original().as_str() {
            Some("module") => {}
            Some("output.text") => code.extend([
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("text".into()),
            ]),
            Some("output.value") => code.extend([
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("value".into()),
            ]),
            Some("output.bytes") => code.extend([
                Instruction::GetMember("output".into()),
                Instruction::CheckModuleOutput,
                Instruction::GetMember("bytes".into()),
            ]),
            _ => code.push(Instruction::EvaluateRawQuery(expression)),
        }
        code.push(Instruction::EncodePklBinary);
        code.push(Instruction::Return);
        let entry = program.alloc_block(code, BlockMetadata::default())?;
        program.seal_code_segment();
        self.public_operation_entries.insert(entry);
        Ok(entry)
    }

    /// Appends a public operation returning a module's `output.bytes` directly.
    pub(crate) fn append_public_output_bytes_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
    ) -> Result<usize, RuntimeError> {
        crate::host::resources::validate_module_key_uri(&source.uri)?;
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let code = vec![
            Instruction::LoadPrimary(Arc::new(source)),
            Instruction::GetMember("output".into()),
            Instruction::CheckModuleOutput,
            Instruction::GetMember("bytes".into()),
            Instruction::Return,
        ];
        let entry = program.alloc_block(code, BlockMetadata::default())?;
        program.seal_code_segment();
        self.public_operation_entries.insert(entry);
        Ok(entry)
    }

    /// Appends a public operation loading a primary module and returning the module object itself.
    pub(crate) fn append_public_load_module_operation(
        &mut self,
        source: crate::modules::ModuleSourceSpec,
    ) -> Result<usize, RuntimeError> {
        crate::host::resources::validate_module_key_uri(&source.uri)?;
        let program = Arc::make_mut(&mut self.program);
        program.seal_code_segment();
        let code = vec![
            Instruction::LoadPrimary(Arc::new(source)),
            Instruction::Return,
        ];
        let entry = program.alloc_block(code, BlockMetadata::default())?;
        program.seal_code_segment();
        self.public_operation_entries.insert(entry);
        Ok(entry)
    }
    fn fresh_module_run(
        &self,
        uri: PklString,
        purpose: ModulePurpose,
        supplied: Option<SourceText>,
    ) -> ModuleRun {
        ModuleRun {
            spec: ImportSpec {
                authored: uri.clone(),
                absolute_uri: uri.clone(),
                original_uri: uri,
                span: Span::default(),
                unit: UnitId(u32::MAX),
            },
            site: None,
            purpose,
            supplied,
            internal_key: false,
            phase: ModulePhase::SelectKey,
            key: None,
            delegate_key: None,
            resolved_key: None,
            cached: true,
            resolved: None,
            receiver: None,
            initializing: false,
            parsed: None,
            source: None,
        }
    }
    pub(super) fn begin_primary(
        &mut self,
        source: Arc<crate::modules::ModuleSourceSpec>,
    ) -> Result<(), RuntimeError> {
        self.public_primary = None;
        if self.pure_standard_host
            && source.contents.is_none()
            && source
                .uri
                .as_str()
                .is_some_and(crate::host::resources::is_clean_file_uri)
        {
            if let Some(cached) = self.modules_original.get_ref(&source.uri).cloned() {
                let value = self.module_cached(cached)?;
                let receiver = value.as_object()?;
                self.public_primary = Some(receiver);
                self.stack.push(value);
                self.state = State::Running;
                return Ok(());
            }
            let key = ModuleKey {
                id: ModuleKeyId(next_key_id()?),
                uri: source.uri.clone(),
                kind: ModuleKeyKind::StandardFile,
                handle: PklString::default(),
            };
            let mut run = self.fresh_module_run(source.uri.clone(), ModulePurpose::Primary, None);
            run.key = Some(key.clone());
            run.phase = ModulePhase::Resolve;
            self.frames.last_mut().unwrap().module_load = Some(Box::new(run));
            return self.wait_module(ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id: ResolvedKeyId(next_key_id()?),
                phase: crate::modules::ResolutionPhase::Primary,
            });
        }
        let (_, normalized) = crate::host::resources::prepare_module_uri(&source.uri, None)?;
        let mut run = self.fresh_module_run(
            source.uri.clone(),
            ModulePurpose::Primary,
            source.contents.clone(),
        );
        run.spec.original_uri = normalized.clone();
        self.frames.last_mut().unwrap().module_load = Some(Box::new(run));
        let id = ModuleKeyId(next_key_id()?);
        if source.uri == "repl:text" && source.contents.is_some() {
            self.frames
                .last_mut()
                .unwrap()
                .module_load
                .as_mut()
                .unwrap()
                .internal_key = true;
            return self.selected_module_key(Ok(ModuleKey {
                id,
                uri: source.uri.clone(),
                kind: ModuleKeyKind::Synthetic,
                handle: "".into(),
            }));
        }
        self.wait_module(ModuleRequestKind::SelectKey {
            id,
            uri: normalized,
            phase: SelectionPhase::Primary,
        })
    }
    pub(super) fn begin_query(&mut self, expression: SourceText) -> Result<(), RuntimeError> {
        self.begin_query_with_purpose(expression, ModulePurpose::Query)
    }
    pub(super) fn begin_raw_query(&mut self, expression: SourceText) -> Result<(), RuntimeError> {
        self.begin_query_with_purpose(expression, ModulePurpose::RawQuery)
    }
    fn begin_query_with_purpose(
        &mut self,
        expression: SourceText,
        purpose: ModulePurpose,
    ) -> Result<(), RuntimeError> {
        let receiver = self.pop()?.as_object()?;
        let mut run = self.fresh_module_run("repl:text".into(), purpose, Some(expression));
        run.receiver = Some(receiver);
        run.phase = ModulePhase::Resolve;
        run.internal_key = true;
        let key = ModuleKey {
            id: ModuleKeyId(next_key_id()?),
            uri: "repl:text".into(),
            kind: ModuleKeyKind::Synthetic,
            handle: "".into(),
        };
        run.key = Some(key.clone());
        self.frames.last_mut().unwrap().module_load = Some(Box::new(run));
        self.wait_module(ModuleRequestKind::ResolveKey {
            key,
            resolved_key_id: ResolvedKeyId(next_key_id()?),
            phase: crate::modules::ResolutionPhase::Query,
        })
    }
    fn compile_public_query(&mut self) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .module_load
            .take()
            .ok_or_else(|| invalid("missing query continuation"))?;
        let raw_value = run.purpose == ModulePurpose::RawQuery;
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing query receiver"))?;
        let unit = self
            .heap
            .get(receiver)?
            .module_unit
            .ok_or_else(|| invalid("query receiver has no module unit"))?;
        let expression = run
            .supplied
            .ok_or_else(|| invalid("missing query source"))?;
        let power_assertions = self.power_assertions;
        if let Some(pos) = self.query_cache.iter().position(|cached| {
            cached.unit == unit
                && cached.raw_value == raw_value
                && cached.power_assertions == power_assertions
                && cached.expression == *expression.original()
                && self.heap.executable_is_live(cached.owner)
        }) {
            let cached = self.query_cache[pos].clone();
            self.push_module_initializer(cached.entry, None);
            let frame = self.frames.last_mut().unwrap();
            frame.environment.executable_owner = Some(cached.owner);
            frame.environment.code_owner = Some(cached.owner);
            frame.environment.scopes = crate::heap::ScopeList::from_single(Scope {
                receiver,
                owner: receiver,
            });
            self.state = State::Running;
            return Ok(());
        }
        let declaration = self.program.units[unit.0 as usize].clone();
        let source = declaration
            .source
            .ok_or_else(|| invalid("query module has no source"))?;
        let program = Arc::make_mut(&mut self.program);
        program.align_metadata()?;
        let arena = query_lifecycle::CodeArena::begin(program);
        let blocks_start = program.blocks.next_id();
        let objects_start = program.objects.next_id();
        let recipes_start = program.type_recipes.next_id();
        let units_start = program.units.next_id();
        let declaration_base = program.next_declaration;
        let cacheable_query =
            expression.len_bytes() <= 16 * 1024 && source.text.len_bytes() <= 256 * 1024;
        let expr_hash = if cacheable_query {
            source_text_hash(&expression)
        } else {
            0
        };
        let mod_hash = if cacheable_query {
            source_text_hash(&source.text)
        } else {
            0
        };

        let cached_hit = if cacheable_query
            && let Ok(cache) = compiled_query_cache().lock()
            && let Some(cached) = cache.iter().rev().find(|e| {
                e.expr_hash == expr_hash
                    && e.module_text_hash == mod_hash
                    && e.unit_id == unit.0
                    && e.raw_value == raw_value
                    && e.power_assertions == power_assertions
                    && e.blocks_start == blocks_start
                    && e.objects_start == objects_start
                    && e.recipes_start == recipes_start
                    && e.declaration_base == declaration_base
                    && e.module_uri == source.uri
                    && e.module_name == source.module_name
                    && e.expr_text == expression
                    && e.module_text == source.text
            }) {
            Some((
                cached.entry,
                cached.next_declaration_after,
                Arc::clone(&cached.blocks),
                Arc::clone(&cached.metadata),
                Arc::clone(&cached.objects),
                Arc::clone(&cached.type_recipes),
                Arc::clone(&cached.declaration_sources),
                cached.reflection_annotations.clone(),
                cached.alias_definitions.clone(),
                cached.constraint_origins.clone(),
            ))
        } else {
            None
        };

        let entry = if let Some((
            entry,
            next_decl_after,
            blocks_seg,
            metadata_seg,
            objects_seg,
            recipes_seg,
            decl_sources,
            refl_annots,
            alias_defs,
            constr_origins,
        )) = cached_hit
        {
            let blocks_end = blocks_start + blocks_seg.len();
            program.blocks.push_sealed_segment(blocks_seg)?;
            program.metadata.push_sealed_segment(metadata_seg)?;
            program.objects.push_sealed_segment(objects_seg)?;
            program.type_recipes.push_sealed_segment(recipes_seg)?;
            program.next_declaration = next_decl_after;
            if !decl_sources.is_empty() {
                program
                    .extra_declaration_sources
                    .push((blocks_start..blocks_end, decl_sources));
            }
            if !refl_annots.is_empty() {
                program.reflection_annotations.extend(refl_annots);
            }
            if !alias_defs.is_empty() {
                program.alias_definitions.extend(alias_defs);
            }
            if !constr_origins.is_empty() {
                program.constraint_origins.extend(constr_origins);
            }
            entry
        } else {
            let parsed_owned;
            let module: &crate::syntax::Module = match &declaration.syntax {
                Some(syntax) => syntax,
                None => {
                    parsed_owned = crate::parser::parse_module_for_evaluation(&source.text)
                        .map_err(|failure| {
                            self.module_parse_error(*failure, source.clone(), Some("".into()))
                        })?;
                    &parsed_owned
                }
            };
            let program = Arc::make_mut(&mut self.program);
            let before_decl_len = program.declaration_sources.len();
            let before_refl_len = program.reflection_annotations.len();
            let before_alias_len = program.alias_definitions.len();
            let before_constr_len = program.constraint_origins.len();
            let before_base_mixin = program.base_mixin_declaration;
            let entry = if raw_value {
                session_expressions::compile_session_expression_value_for_unit(
                    program,
                    module,
                    &expression,
                    CompileOptions {
                        fuse_constant_add: true,
                    },
                    power_assertions,
                    unit,
                )
            } else {
                session_expressions::compile_session_expression_for_unit(
                    program,
                    module,
                    &expression,
                    CompileOptions {
                        fuse_constant_add: true,
                    },
                    power_assertions,
                    unit,
                )
            }
            .map_err(|failure| match failure {
                SessionExpressionError::Frontend(error) => imported_diagnostic(
                    *error,
                    Arc::new(RuntimeSource {
                        text: expression.clone(),
                        uri: "repl:text".into(),
                        module_name: "".into(),
                    }),
                    "repl:text".into(),
                ),
                SessionExpressionError::Runtime(error) => *error,
            })?;
            let blocks_end = program.blocks.next_id();
            let objects_end = program.objects.next_id();
            let recipes_end = program.type_recipes.next_id();
            if cacheable_query
                && program.units.next_id() == units_start
                && blocks_end.saturating_sub(blocks_start) <= 128
                && program.base_mixin_declaration == before_base_mixin
                && program.metadata.next_id() == blocks_end
            {
                let decl_delta: Vec<_> = program
                    .declaration_sources
                    .range(blocks_start..blocks_end)
                    .map(|(&k, v)| (k, v.clone()))
                    .collect();
                let refl_delta: Vec<_> = program
                    .reflection_annotations
                    .iter()
                    .filter(|(_, block)| **block >= blocks_start && **block < blocks_end)
                    .map(|(k, &v)| (k.clone(), v))
                    .collect();
                let alias_delta: Vec<_> = program
                    .alias_definitions
                    .range(declaration_base..program.next_declaration)
                    .map(|(&k, v)| (k, v.clone()))
                    .collect();
                let constr_delta: Vec<_> = program
                    .constraint_origins
                    .range(blocks_start..blocks_end)
                    .map(|(&k, v)| (k, v.clone()))
                    .collect();
                if program.declaration_sources.len() == before_decl_len + decl_delta.len()
                    && program.reflection_annotations.len() == before_refl_len + refl_delta.len()
                    && program.alias_definitions.len() == before_alias_len + alias_delta.len()
                    && program.constraint_origins.len() == before_constr_len + constr_delta.len()
                    && let (
                        Some(blocks_seg),
                        Some(metadata_seg),
                        Some(objects_seg),
                        Some(recipes_seg),
                    ) = (
                        program.blocks.slice_segment(blocks_start, blocks_end),
                        program.metadata.slice_segment(blocks_start, blocks_end),
                        program.objects.slice_segment(objects_start, objects_end),
                        program
                            .type_recipes
                            .slice_segment(recipes_start, recipes_end),
                    )
                    && let Ok(mut cache) = compiled_query_cache().lock()
                {
                    if cache.len() >= 16 {
                        cache.remove(0);
                    }
                    cache.push(CachedCompiledQuery {
                        expr_hash,
                        expr_text: expression.clone(),
                        module_text_hash: mod_hash,
                        module_text: source.text.clone(),
                        module_uri: source.uri.clone(),
                        module_name: source.module_name.clone(),
                        raw_value,
                        power_assertions,
                        unit_id: unit.0,
                        blocks_start,
                        objects_start,
                        recipes_start,
                        declaration_base,
                        next_declaration_after: program.next_declaration,
                        entry,
                        blocks: blocks_seg,
                        metadata: metadata_seg,
                        objects: objects_seg,
                        type_recipes: recipes_seg,
                        declaration_sources: Arc::from(decl_delta),
                        reflection_annotations: refl_delta,
                        alias_definitions: alias_delta,
                        constraint_origins: constr_delta,
                    });
                }
            }
            entry
        };
        self.push_module_initializer(entry, None);
        // The synthetic query owns its executable state for this operation.
        // Frames and escaped environments trace this owner; the evaluator's
        // permanent static-root table must not retain a completed query site.
        let owner = self.heap.allocate_executable();
        self.code_arenas
            .push(arena.finish(Arc::make_mut(&mut self.program), owner, true));
        self.declaration_roots_dirty = true;
        if self.query_cache.len() >= 64 {
            self.query_cache.remove(0);
        }
        self.query_cache.push(CachedQuery {
            unit,
            expression: expression.original().clone(),
            raw_value,
            power_assertions,
            entry,
            owner,
        });
        let frame = self.frames.last_mut().unwrap();
        frame.environment.executable_owner = Some(owner);
        frame.environment.code_owner = Some(owner);
        frame.environment.scopes = crate::heap::ScopeList::from_single(Scope {
            receiver,
            owner: receiver,
        });
        self.state = State::Running;
        Ok(())
    }
    pub(crate) fn public_primary_source(&self) -> Option<Arc<RuntimeSource>> {
        self.public_primary
            .and_then(|id| self.heap.get(id).ok())
            .and_then(|object| object.module_unit)
            .and_then(|unit| self.program.units[unit.0 as usize].source.clone())
    }
    pub(super) fn current_module_name(&self) -> &PklString {
        self.frames
            .last()
            .and_then(|frame| self.program.metadata.get(frame.block))
            .and_then(|metadata| metadata.source.as_ref())
            .map_or(&self.program.module_name, |source| &source.module_name)
    }
    pub(super) fn module_cached(&mut self, cached: CachedModule) -> Result<Value, RuntimeError> {
        match cached {
            CachedModule::Value(id) => Ok(Value::Object(id)),
            CachedModule::Failed(error) => {
                // A cached exception is thrown by ModuleCache, whose boundary
                // has no guest call node, just like source initialization.
                if let Some(frame) = self.frames.last_mut() {
                    frame.callsite_hidden = true;
                }
                Err(error)
            }
        }
    }

    pub(super) fn begin_import(&mut self, spec: ImportSpec) -> Result<(), RuntimeError> {
        self.begin_import_node(spec, true)
    }
    pub(super) fn begin_glob_member_import(
        &mut self,
        spec: ImportSpec,
    ) -> Result<(), RuntimeError> {
        self.begin_import_node(spec, false)
    }
    fn begin_import_node(
        &mut self,
        spec: ImportSpec,
        cache_node: bool,
    ) -> Result<(), RuntimeError> {
        let owner = self.current_executable()?;
        let frame = self.frames.last().expect("import frame");
        let site = (frame.block, frame.pc - 1);
        if cache_node && let Some(receiver) = self.heap.imported_module(owner, site)? {
            self.stack.push(Value::Object(receiver));
            return Ok(());
        }
        let importer = self
            .program
            .units
            .get(spec.unit.0 as usize)
            .and_then(|unit| unit.identity.clone())
            .ok_or_else(|| {
                RuntimeError::new(
                    ErrorKind::Unsupported,
                    "Module imports require a registered declaration ModuleKey.",
                )
            })?;
        if self.pure_standard_host
            && spec
                .original_uri
                .as_str()
                .is_some_and(crate::host::resources::is_clean_file_uri)
            && let Ok(from) = crate::host::resources::module_trust_level(&importer.original_uri)
            && let Ok(to) = crate::host::resources::module_trust_level(&spec.absolute_uri)
            && from >= to
        {
            if let Some(cached) = self.modules_original.get_ref(&spec.original_uri).cloned() {
                let value = self.module_cached(cached)?;
                let receiver = value.as_object()?;
                if cache_node {
                    self.heap.publish_imported_module(owner, site, receiver)?;
                }
                self.stack.push(value);
                self.state = State::Running;
                return Ok(());
            }
            let key = ModuleKey {
                id: ModuleKeyId(next_key_id()?),
                uri: spec.original_uri.clone(),
                kind: ModuleKeyKind::StandardFile,
                handle: PklString::default(),
            };
            let import_uri = spec.absolute_uri.clone();
            let original_uri = spec.original_uri.clone();
            self.frames.last_mut().expect("import frame").module_load = Some(Box::new(ModuleRun {
                site: cache_node.then_some((owner, site.0, site.1)),
                purpose: ModulePurpose::Import,
                supplied: None,
                internal_key: false,
                phase: ModulePhase::Resolve,
                key: Some(key.clone()),
                delegate_key: None,
                resolved_key: None,
                cached: true,
                spec,
                resolved: None,
                receiver: None,
                initializing: false,
                parsed: None,
                source: None,
            }));
            return self.wait_module(ModuleRequestKind::Resolve {
                key,
                resolved_key_id: ResolvedKeyId(next_key_id()?),
                importer,
                import_uri,
                original_uri,
            });
        }
        let request = ModuleRequestKind::CheckImport {
            importer,
            import_uri: spec.absolute_uri.clone(),
        };
        self.frames.last_mut().expect("import frame").module_load = Some(Box::new(ModuleRun {
            site: cache_node.then_some((owner, site.0, site.1)),
            purpose: ModulePurpose::Import,
            supplied: None,
            internal_key: false,
            phase: ModulePhase::ImportCheck,
            key: None,
            delegate_key: None,
            resolved_key: None,
            cached: true,
            spec,
            resolved: None,
            receiver: None,
            initializing: false,
            parsed: None,
            source: None,
        }));
        self.wait_module(request)
    }

    fn checked_import(
        &mut self,
        result: Result<(), crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last()
            .and_then(|frame| frame.module_load.as_ref())
            .ok_or_else(|| invalid("missing import admission continuation"))?;
        if run.phase != ModulePhase::ImportCheck {
            return Err(invalid("unexpected import admission phase"));
        }
        result.map_err(|error| error.into_runtime(&run.spec.absolute_uri))?;
        let uri = run.spec.original_uri.clone();
        self.frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap()
            .phase = ModulePhase::SelectKey;
        self.wait_module(ModuleRequestKind::SelectKey {
            id: ModuleKeyId(next_key_id()?),
            uri,
            phase: SelectionPhase::RuntimeImport,
        })
    }

    fn selected_module_key(
        &mut self,
        result: Result<ModuleKey, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing key selection continuation"))?;
        let key = result.map_err(|error| error.into_factory_runtime(&run.spec.absolute_uri))?;
        let key =
            if run.purpose == ModulePurpose::Primary && run.supplied.is_some() && !run.internal_key
            {
                run.delegate_key = Some(key.clone());
                run.internal_key = true;
                ModuleKey {
                    id: ModuleKeyId(next_key_id()?),
                    uri: key.uri,
                    kind: ModuleKeyKind::SuppliedText,
                    handle: "".into(),
                }
            } else {
                key
            };
        run.spec.original_uri = key.uri.clone();
        run.key = Some(key.clone());
        if key.kind == ModuleKeyKind::StandardLibrary {
            match crate::modules::stdlib_kind(&key.uri) {
                Some(crate::modules::StdlibKind::Missing) => {
                    return Err(self.stdlib_source(&key.uri).unwrap_err());
                }
                Some(crate::modules::StdlibKind::Singleton(canonical)) => {
                    if let Some(cached) = self.modules_stdlib.get(canonical).cloned() {
                        let value = self.module_cached(cached)?;
                        return self.finish_import(value);
                    }
                    return self.initialize_bundled(ResolvedModule {
                        original_uri: canonical.into(),
                        resolved_uri: canonical.into(),
                        source_uri: canonical.into(),
                    });
                }
                Some(crate::modules::StdlibKind::Cached) => {}
                None => return Err(invalid("StandardLibrary key requires the pkl scheme")),
            }
        }
        let internal_key = run.internal_key;
        self.frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap()
            .phase = ModulePhase::CachePolicy;
        if internal_key {
            return self.queried_module_cache_policy(Ok(key.kind == ModuleKeyKind::SuppliedText));
        }
        self.wait_module(ModuleRequestKind::QueryCachePolicy { key })
    }

    fn queried_module_cache_policy(
        &mut self,
        result: Result<bool, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing cache policy continuation"))?;
        run.cached = result.map_err(|error| error.into_runtime(&run.spec.original_uri))?;
        let spec = run.spec.clone();
        let key = run
            .key
            .clone()
            .ok_or_else(|| invalid("missing selected ModuleKey"))?;
        if run.cached
            && let Some(cached) = self.modules_original.get(&key.uri).cloned()
        {
            let value = self.module_cached(cached)?;
            return self.finish_import(value);
        }
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap();
        run.phase = ModulePhase::Resolve;
        if run.internal_key && key.kind == ModuleKeyKind::SuppliedText {
            return self.resolved_module(Ok(ResolvedModuleKey {
                id: ResolvedKeyId(next_key_id()?),
                key: key.clone(),
                handle: "".into(),
                module: ResolvedModule {
                    original_uri: key.uri.clone(),
                    resolved_uri: key.uri.clone(),
                    source_uri: key.uri.clone(),
                },
            }));
        }
        let resolved_key_id = ResolvedKeyId(next_key_id()?);
        if run.purpose == ModulePurpose::Primary {
            return self.wait_module(ModuleRequestKind::ResolveKey {
                key,
                resolved_key_id,
                phase: crate::modules::ResolutionPhase::Primary,
            });
        }
        let importer = self.program.units[spec.unit.0 as usize]
            .identity
            .clone()
            .ok_or_else(|| invalid("import identity disappeared"))?;
        self.wait_module(ModuleRequestKind::Resolve {
            key,
            resolved_key_id,
            importer,
            import_uri: spec.absolute_uri,
            original_uri: spec.original_uri,
        })
    }

    fn finish_import(&mut self, value: Value) -> Result<(), RuntimeError> {
        let receiver = value.as_object()?;
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.take())
            .ok_or_else(|| invalid("missing import result continuation"))?;
        if let Some(site) = run.site {
            self.heap
                .publish_imported_module(site.0, (site.1, site.2), receiver)?;
        }
        if run.purpose == ModulePurpose::Primary {
            self.public_primary = Some(receiver);
        }
        self.stack.push(value);
        self.state = State::Running;
        Ok(())
    }

    fn initialize_bundled(&mut self, resolved: ResolvedModule) -> Result<(), RuntimeError> {
        let receiver = self.heap.allocate(Object::default());
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing bundled import continuation"))?;
        run.resolved = Some(resolved.clone());
        run.receiver = Some(receiver);
        run.initializing = true;
        run.phase = ModulePhase::Initializer;
        let entry = self.stdlib_entry(&resolved.original_uri)?;
        self.state = State::Running;
        self.push_module_initializer(entry, Some(receiver));
        Ok(())
    }

    pub(super) fn wait_module(&mut self, kind: ModuleRequestKind) -> Result<(), RuntimeError> {
        if self.pure_standard_host
            && self.inline_module_depth < 16
            && !self.frames.last().is_some_and(|frame| {
                frame.glob.is_some()
                    || frame
                        .analysis
                        .as_ref()
                        .is_some_and(|run| run.awaiting_host())
            })
            && let Some(result) = self.try_inline_standard_module_request(&kind)
        {
            return result;
        }
        let id = self.next_request;
        self.next_request = id
            .checked_add(1)
            .ok_or_else(|| invalid("Host request identity exhausted"))?;
        self.state = State::Waiting(PendingRequest::Module(ModuleRequest { id, kind }));
        Ok(())
    }

    fn try_inline_standard_module_request(
        &mut self,
        kind: &ModuleRequestKind,
    ) -> Option<Result<(), RuntimeError>> {
        match kind {
            ModuleRequestKind::SelectKey { id, uri, phase } => {
                let selected_kind = if uri
                    .as_str()
                    .is_some_and(crate::host::resources::is_clean_file_uri)
                {
                    ModuleKeyKind::StandardFile
                } else if uri.starts_with("pkl:")
                    && crate::host::resources::validate_module_key_uri(uri).is_ok()
                {
                    ModuleKeyKind::StandardLibrary
                } else {
                    return None;
                };
                let key = ModuleKey {
                    id: *id,
                    uri: uri.clone(),
                    kind: selected_kind,
                    handle: PklString::default(),
                };
                self.inline_module_depth += 1;
                let res = if *phase == SelectionPhase::ClausePreparation {
                    self.selected_clause_key(Ok(key))
                } else {
                    self.selected_module_key(Ok(key))
                };
                self.inline_module_depth -= 1;
                Some(res)
            }
            ModuleRequestKind::QueryCachePolicy { key }
                if matches!(
                    key.kind,
                    ModuleKeyKind::StandardLibrary
                        | ModuleKeyKind::StandardFile
                        | ModuleKeyKind::StandardHttp
                        | ModuleKeyKind::StandardGenericUrl
                ) || key.handle == "builtin:modulepath" =>
            {
                self.inline_module_depth += 1;
                let res = self.queried_module_cache_policy(Ok(true));
                self.inline_module_depth -= 1;
                Some(res)
            }
            ModuleRequestKind::QueryFragmentPaths { key }
                if matches!(
                    key.kind,
                    ModuleKeyKind::StandardLibrary
                        | ModuleKeyKind::StandardFile
                        | ModuleKeyKind::StandardHttp
                        | ModuleKeyKind::StandardGenericUrl
                ) || key.handle == "builtin:modulepath" =>
            {
                let has_fragment =
                    key.uri.starts_with("package:") || key.uri.starts_with("projectpackage:");
                let is_name_phase = self
                    .frames
                    .last()
                    .and_then(|frame| frame.module_load.as_ref())
                    .is_some_and(|run| run.phase == ModulePhase::Name);
                self.inline_module_depth += 1;
                let res = if is_name_phase {
                    self.resolved_module_name(Ok(has_fragment))
                } else {
                    self.resolved_name_fragment_paths(Ok(has_fragment))
                };
                self.inline_module_depth -= 1;
                Some(res)
            }
            ModuleRequestKind::Capabilities { key, module }
                if key
                    .as_ref()
                    .is_some_and(|k| k.kind == ModuleKeyKind::StandardFile)
                    || (key.is_none()
                        && module
                            .original_uri
                            .as_str()
                            .is_some_and(crate::host::resources::is_clean_file_uri)) =>
            {
                self.inline_module_depth += 1;
                let res = self.resolved_capabilities(Ok(crate::modules::ModuleCapabilities {
                    local: true,
                    hierarchical: true,
                    fragment_paths: false,
                }));
                self.inline_module_depth -= 1;
                Some(res)
            }
            ModuleRequestKind::ResolveImportUri { key, authored_uri }
                if !authored_uri.starts_with("@")
                    && !key.uri.starts_with("package:")
                    && !key.uri.starts_with("projectpackage:") =>
            {
                let resolved_uri =
                    crate::host::resources::resolve_module_key_uri(authored_uri, &key.uri).ok()?;
                crate::host::resources::validate_module_key_uri(&resolved_uri).ok()?;
                self.inline_module_depth += 1;
                let res = self.resolved_import_uri(Ok(resolved_uri));
                self.inline_module_depth -= 1;
                Some(res)
            }
            ModuleRequestKind::CheckImport {
                importer,
                import_uri,
            } => {
                let from =
                    crate::host::resources::module_trust_level(&importer.original_uri).ok()?;
                let to = crate::host::resources::module_trust_level(import_uri).ok()?;
                if from < to {
                    return None;
                }
                self.inline_module_depth += 1;
                let res = self.checked_import(Ok(()));
                self.inline_module_depth -= 1;
                Some(res)
            }
            _ => None,
        }
    }

    pub fn complete_module(
        &mut self,
        id: u64,
        completion: ModuleCompletion,
    ) -> Result<(), RuntimeError> {
        let State::Waiting(PendingRequest::Module(request)) = &self.state else {
            return Err(stale_module());
        };
        if request.id != id
            || !matches!(
                (&request.kind, &completion),
                (
                    ModuleRequestKind::CheckImport { .. },
                    ModuleCompletion::ImportChecked(_)
                ) | (
                    ModuleRequestKind::SelectKey { .. },
                    ModuleCompletion::KeySelected(_)
                ) | (
                    ModuleRequestKind::QueryCachePolicy { .. },
                    ModuleCompletion::CachePolicy(_)
                ) | (
                    ModuleRequestKind::QueryFragmentPaths { .. },
                    ModuleCompletion::FragmentPaths(_)
                ) | (
                    ModuleRequestKind::ResolveImportUri { .. },
                    ModuleCompletion::ImportUri(_)
                ) | (
                    ModuleRequestKind::Capabilities { .. },
                    ModuleCompletion::Capabilities(_)
                ) | (
                    ModuleRequestKind::HasElement { .. },
                    ModuleCompletion::HasElement(_)
                ) | (
                    ModuleRequestKind::Resolve { .. } | ModuleRequestKind::ResolveKey { .. },
                    ModuleCompletion::Resolved(_) | ModuleCompletion::KeyResolved(_)
                ) | (
                    ModuleRequestKind::LoadSource { .. },
                    ModuleCompletion::Source(_)
                )
            )
        {
            return Err(stale_module());
        }
        if matches!(
            &completion,
            ModuleCompletion::ImportChecked(Err(crate::modules::ModuleHostError::NotFound
                | crate::modules::ModuleHostError::NoLoader
                | crate::modules::ModuleHostError::UriSyntax { .. }
                | crate::modules::ModuleHostError::ExternalReaderProcess { .. }
                | crate::modules::ModuleHostError::Io(_)))
        ) {
            return Err(invalid(
                "Import admission does not accept module lookup or I/O failures.",
            ));
        }
        use crate::modules::ModuleHostError as Failure;
        if matches!(
            &completion,
            ModuleCompletion::KeySelected(Err(Failure::NotFound
                | Failure::SecurityManagerException { .. }
                | Failure::EvalError { .. }))
                | ModuleCompletion::FragmentPaths(Err(Failure::NotFound
                    | Failure::NoLoader
                    | Failure::Io(_)
                    | Failure::UriSyntax { .. }
                    | Failure::ExternalReaderProcess { .. }
                    | Failure::SecurityManagerException { .. }
                    | Failure::EvalError { .. }))
                | ModuleCompletion::CachePolicy(Err(Failure::NotFound
                    | Failure::NoLoader
                    | Failure::Io(_)
                    | Failure::UriSyntax { .. }
                    | Failure::ExternalReaderProcess { .. }
                    | Failure::SecurityManagerException { .. }
                    | Failure::EvalError { .. }))
                | ModuleCompletion::Resolved(Err(Failure::NoLoader))
                | ModuleCompletion::KeyResolved(Err(Failure::NoLoader))
                | ModuleCompletion::Source(Err(Failure::NoLoader
                    | Failure::UriSyntax { .. }
                    | Failure::ExternalReaderProcess { .. }))
                | ModuleCompletion::Capabilities(Err(Failure::NoLoader))
                | ModuleCompletion::HasElement(Err(Failure::NoLoader))
                | ModuleCompletion::ImportUri(Err(Failure::NoLoader | Failure::NotFound))
        ) {
            return Err(invalid(
                "Module Host failure does not belong to this phase.",
            ));
        }
        match (&request.kind, &completion) {
            (ModuleRequestKind::ResolveImportUri { .. }, ModuleCompletion::ImportUri(Ok(uri))) => {
                crate::host::resources::validate_module_key_uri(uri)?;
            }
            (ModuleRequestKind::SelectKey { id, .. }, ModuleCompletion::KeySelected(Ok(key))) => {
                if key.id != *id {
                    return Err(stale_module());
                }
                crate::host::resources::validate_module_key_uri(&key.uri)?;
                if key.kind == ModuleKeyKind::StandardLibrary
                    && crate::modules::stdlib_kind(&key.uri).is_none()
                {
                    return Err(invalid("StandardLibrary key requires the pkl scheme"));
                }
            }
            (
                ModuleRequestKind::Resolve {
                    key,
                    resolved_key_id,
                    ..
                }
                | ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                },
                ModuleCompletion::KeyResolved(Ok(resolved)),
            ) => {
                if resolved.id != *resolved_key_id || resolved.key != *key {
                    return Err(stale_module());
                }
                validate_resolution(key, &resolved.module)?;
            }
            (
                ModuleRequestKind::Resolve { key, .. } | ModuleRequestKind::ResolveKey { key, .. },
                ModuleCompletion::Resolved(Ok(resolved)),
            ) => validate_resolution(key, resolved)?,
            _ => {}
        }
        if matches!(
            &completion,
            ModuleCompletion::Source(Err(
                crate::modules::ModuleHostError::SecurityManagerException { .. }
            ))
        ) {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "SecurityManagerException is only supported for module resolution.",
            ));
        }
        if matches!(
            &completion,
            ModuleCompletion::Capabilities(Err(
                crate::modules::ModuleHostError::SecurityManagerException { .. }
            ))
        ) {
            return Err(RuntimeError::new(
                ErrorKind::Host,
                "SecurityManagerException is not supported for ModuleKey capability getters.",
            ));
        }
        // Validate the public request and global key identities above before
        // translating them back into the static continuation's private IDs.
        if self.frames.last().is_some_and(|frame| {
            frame
                .analysis
                .as_ref()
                .is_some_and(|run| run.awaiting_host())
        }) {
            let result = self.complete_analysis_module(completion);
            if let Err(error) = result {
                self.fail(error);
            }
            return Ok(());
        }
        if self.frames.last().is_some_and(|frame| frame.glob.is_some()) {
            let result = self.complete_glob_module(completion);
            if let Err(error) = result {
                self.fail(error);
            }
            return Ok(());
        }
        let result = match completion {
            ModuleCompletion::ImportChecked(result) => self.checked_import(result),
            ModuleCompletion::KeySelected(result) => {
                if matches!(
                    &request.kind,
                    ModuleRequestKind::SelectKey {
                        phase: SelectionPhase::ClausePreparation,
                        ..
                    }
                ) {
                    self.selected_clause_key(result)
                } else {
                    self.selected_module_key(result)
                }
            }
            ModuleCompletion::FragmentPaths(result) => {
                if self
                    .frames
                    .last()
                    .and_then(|frame| frame.module_load.as_ref())
                    .is_some_and(|run| run.phase == ModulePhase::Name)
                {
                    self.resolved_module_name(result)
                } else {
                    self.resolved_name_fragment_paths(result)
                }
            }
            ModuleCompletion::ImportUri(result) => self.resolved_import_uri(result),
            ModuleCompletion::CachePolicy(result) => self.queried_module_cache_policy(result),
            ModuleCompletion::Capabilities(result) => self.resolved_capabilities(result),
            ModuleCompletion::HasElement(result) => self.resolved_element(result),
            ModuleCompletion::Resolved(result) => {
                let (ModuleRequestKind::Resolve {
                    key,
                    resolved_key_id,
                    ..
                }
                | ModuleRequestKind::ResolveKey {
                    key,
                    resolved_key_id,
                    ..
                }) = &request.kind
                else {
                    unreachable!()
                };
                let key = key.clone();
                let id = *resolved_key_id;
                self.resolved_module(result.map(|module| ResolvedModuleKey {
                    id,
                    key,
                    module,
                    handle: "".into(),
                }))
            }
            ModuleCompletion::KeyResolved(result) => self.resolved_module(result),
            ModuleCompletion::Source(result) => self.loaded_module(result),
        };
        if let Err(error) = result {
            self.fail(error);
        }
        Ok(())
    }

    fn resolved_module(
        &mut self,
        result: Result<ResolvedModuleKey, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing import continuation"))?;
        let resolved_key =
            result.map_err(|error| error.into_resolution_runtime(&run.spec.original_uri))?;
        let resolved = resolved_key.module.clone();
        if resolved.original_uri != run.spec.original_uri {
            return Err(invalid("module resolver changed original key"));
        }
        if matches!(run.purpose, ModulePurpose::Query | ModulePurpose::RawQuery) {
            return self.compile_public_query();
        }
        if run.cached
            && let Some(cached) = self.modules_resolved.get(&resolved.resolved_uri).cloned()
        {
            let value = self.module_cached(cached)?;
            // A resolved-key hit does not seed the original alias cache.
            return self.finish_import(value);
        }
        if resolved_key.key.kind == ModuleKeyKind::StandardLibrary {
            return self.initialize_bundled(resolved);
        }
        // VmTyped exists before source I/O but is not cache-visible until that
        // source succeeds. A callback can independently load the same key.
        let receiver = self.heap.allocate(Object::default());
        run.receiver = Some(receiver);
        run.resolved = Some(resolved.clone());
        run.resolved_key = Some(resolved_key.clone());
        run.phase = ModulePhase::Source;
        if let Some(text) = run.supplied.clone() {
            return self.loaded_module(Ok(ModuleSource {
                text,
                inferred_name: "".into(),
            }));
        }
        self.wait_module(ModuleRequestKind::LoadSource {
            key: resolved_key,
            resolved,
        })
    }

    fn loaded_module(
        &mut self,
        result: Result<ModuleSource, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing module source continuation"))?;
        let resolved = run
            .resolved
            .clone()
            .ok_or_else(|| invalid("missing resolved key"))?;
        // From this boundary onward failure is cached, including source errors.
        run.initializing = true;
        run.phase = ModulePhase::Initializer;
        let input = result.map_err(|error| error.into_runtime(&resolved.source_uri))?;
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing empty module"))?;
        let cached = run.cached;
        self.modules_original
            .insert(resolved.original_uri.clone(), CachedModule::Value(receiver));
        self.modules_resolved
            .insert(resolved.resolved_uri.clone(), CachedModule::Value(receiver));
        if cached {
            self.record_cached_module_uris(&resolved.original_uri, &resolved.resolved_uri);
        } else {
            self.record_uncached_module_uris(&resolved.original_uri, &resolved.resolved_uri);
        }
        let mut source = Arc::new(RuntimeSource {
            text: input.text.clone(),
            uri: resolved.source_uri.clone(),
            module_name: input.inferred_name,
        });
        // An ImportNode's guest call frame supplies the first visible member
        // for its parser exception. Primary loading has no such guest frame.
        let frame = self.frames.last().expect("import continuation");
        let metadata = &self.program.metadata[frame.block];
        let import_member = (frame.module_load.as_ref().expect("module load").purpose
            == ModulePurpose::Import)
            .then(|| {
                PklString::from(
                    metadata
                        .source
                        .as_ref()
                        .map_or(&self.program.module_name, |source| &source.module_name),
                )
                .concat(frame.diagnostic_member.as_ref().unwrap_or(&metadata.member))
            });
        // VmLanguage infers the selected key's module name even when parsing
        // fails. Keep the original diagnostic while its name getter suspends.
        let parsed =
            if let Some((cached_resolved, cached_syntax)) = cached_user_resolved(&input.text) {
                Ok(ParsedOrResolved::Resolved(cached_resolved, cached_syntax))
            } else {
                crate::parser::parse_module_for_evaluation(&input.text)
                    .map(|m| ParsedOrResolved::Parsed(Box::new(m)))
                    .map_err(|failure| Box::new((*failure, import_member)))
            };
        if let Some(name) = parsed
            .as_ref()
            .ok()
            .and_then(|parsed| parsed.module().declaration.as_ref())
            .and_then(|d| d.name.as_ref())
        {
            Arc::make_mut(&mut source).module_name = PklString::join(name, ".");
            return self.finish_loaded_parse(parsed, source, receiver);
        }
        let run = self
            .frames
            .last_mut()
            .unwrap()
            .module_load
            .as_mut()
            .unwrap();
        let key = run
            .key
            .clone()
            .ok_or_else(|| invalid("missing loaded ModuleKey"))?;
        let skip_fragment_query = (self.pure_standard_host
            && matches!(
                key.kind,
                ModuleKeyKind::StandardFile
                    | ModuleKeyKind::StandardLibrary
                    | ModuleKeyKind::StandardHttp
            ))
            || !crate::host::resources::module_name_needs_fragment_paths(&key.uri);
        if skip_fragment_query {
            Arc::make_mut(&mut source).module_name =
                crate::host::resources::infer_module_name(&key.uri, false)?;
            return self.finish_loaded_parse(parsed, source, receiver);
        }
        run.parsed = Some(parsed);
        run.source = Some(source);
        run.phase = ModulePhase::Name;
        self.wait_module(ModuleRequestKind::QueryFragmentPaths { key })
    }

    fn resolved_module_name(
        &mut self,
        result: Result<bool, crate::modules::ModuleHostError>,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing module name continuation"))?;
        let key = run
            .key
            .as_ref()
            .ok_or_else(|| invalid("missing name ModuleKey"))?;
        let fragments = result.map_err(|error| error.into_runtime(&key.uri))?;
        let name = crate::host::resources::infer_module_name(&key.uri, fragments)?;
        let mut source = run
            .source
            .take()
            .ok_or_else(|| invalid("missing module name source"))?;
        Arc::make_mut(&mut source).module_name = name;
        let parsed = run
            .parsed
            .take()
            .ok_or_else(|| invalid("missing module name AST"))?;
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing module receiver"))?;
        self.finish_loaded_parse(parsed, source, receiver)
    }

    fn finish_loaded_parse(
        &mut self,
        parsed: ParsedModule,
        source: Arc<RuntimeSource>,
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        match parsed {
            Ok(parsed) => self.compile_loaded_module(parsed, source, receiver),
            Err(error) => {
                let (failure, member) = *error;
                Err(self.module_parse_error(failure, source, member))
            }
        }
    }

    fn module_parse_error(
        &self,
        failure: crate::parser::ModuleParseFailure,
        source: Arc<RuntimeSource>,
        member: Option<PklString>,
    ) -> RuntimeError {
        if let Some(mut error) = failure.minimum_version_error(&source.module_name) {
            if error.kind == ErrorKind::Type
                && let Some(caller) =
                    self.frames.iter().rev().find(|frame| {
                        frame.module_load.as_ref().is_some_and(|run| {
                            run.initializing && run.purpose == ModulePurpose::Import
                        }) && matches!(
                            self.program.blocks[frame.block].get(frame.pc.saturating_sub(1)),
                            Some(Instruction::ImportModule(_))
                        )
                    })
            {
                // Initialization hides this ImportNode's ordinary callsite.
                // The checker explicitly supplies it, with no child parser
                // location; outer authored frames unwind on the existing path.
                let metadata = &self.program.metadata[caller.block];
                if let Some(span) = metadata.spans[caller.pc.saturating_sub(1)] {
                    error.frames.push(RuntimeFrame {
                        source: metadata.source.clone(),
                        span,
                        member: caller
                            .diagnostic_member
                            .clone()
                            .unwrap_or_else(|| metadata.member.clone()),
                        anonymous: false,
                    });
                }
            }
            return error;
        }
        let member = member.unwrap_or_else(|| source.module_name.clone());
        imported_diagnostic(failure.diagnostic, source, member)
    }

    fn compile_loaded_module(
        &mut self,
        parsed: ParsedOrResolved,
        source: Arc<RuntimeSource>,
        receiver: ObjectId,
    ) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.as_mut())
            .ok_or_else(|| invalid("missing module compilation continuation"))?;
        run.phase = ModulePhase::Initializer;
        let resolved = run
            .resolved
            .clone()
            .ok_or_else(|| invalid("missing resolved module"))?;
        let (resolved_ast, cached_syntax) = match parsed {
            ParsedOrResolved::Resolved(cached, syntax) => (cached, Some(syntax)),
            ParsedOrResolved::Parsed(parsed) => {
                let resolved_ast =
                    Arc::new(crate::resolver::resolve_module_for_evaluation(&parsed));
                let syntax = Arc::new(*parsed);
                store_user_resolved(&source.text, Arc::clone(&resolved_ast), Arc::clone(&syntax));
                (resolved_ast, Some(syntax))
            }
        };
        // The evaluator owns one append-only code arena. Clone only while an
        // external prepared-program snapshot shares it; subsequent modules
        // append in place. Failed compilation retracts only its uncommitted tail.
        let power_assertions = self.power_assertions;
        let pure_standard_host = self.pure_standard_host;
        let module_key = self
            .frames
            .last()
            .and_then(|frame| frame.module_load.as_ref())
            .and_then(|run| run.key.clone());
        let delegate_key = self
            .frames
            .last()
            .and_then(|frame| frame.module_load.as_ref())
            .and_then(|run| run.delegate_key.clone());
        let program = Arc::make_mut(&mut self.program);
        program.align_metadata()?;
        let arena = query_lifecycle::CodeArena::begin(program);
        let blocks_start = program.blocks.next_id();
        let objects_start = program.objects.next_id();
        let recipes_start = program.type_recipes.next_id();
        let unit_id_usize = program.units.next_id();
        let declaration_base = program.next_declaration;
        let is_base_module =
            resolved.original_uri == "pkl:base" || resolved.resolved_uri == "pkl:base";
        let is_primary_root = !is_base_module && program.module_template.is_none();
        let cacheable_text = source.text.len_bytes() <= 256 * 1024;
        let text_hash = if cacheable_text {
            source_text_hash(&source.text)
        } else {
            0
        };

        // Compiled-unit segment cache: replay pre-compiled sealed `CodeStorage`
        // segments via O(1) `Arc::clone` when source, URIs, and starting offsets match.
        let cache_key = if cacheable_text && let Ok(unit_id_u32) = u32::try_from(unit_id_usize) {
            Some(CompiledUnitCacheKey {
                hash: text_hash,
                text: &source.text,
                source_uri: &source.uri,
                module_name: &source.module_name,
                has_identity: true,
                original_uri: &resolved.original_uri,
                resolved_uri: &resolved.resolved_uri,
                power_assertions,
                fuse_constant_add: true,
                pure_standard_host,
                is_primary_root,
                is_replacement: false,
                prepared: &[],
                blocks_start,
                objects_start,
                recipes_start,
                unit_id: unit_id_u32,
                declaration_base,
                base_mixin_declaration: program.base_mixin_declaration,
            })
        } else {
            None
        };
        if let Some(key) = &cache_key
            && let Some(mut unit_clone) = try_replay_compiled_unit(program, key)?
        {
            unit_clone.key = module_key;
            unit_clone.delegate_key = delegate_key;
            let entry = unit_clone.entry;
            program.units.try_push(unit_clone)?;
            let owner = self.heap.allocate_executable();
            self.code_arenas.push(arena.finish(program, owner, false));
            self.declaration_roots_dirty = true;
            self.heap.get_mut(receiver)?.environment.code_owner = Some(owner);
            self.state = State::Running;
            self.push_module_initializer(entry, Some(receiver));
            self.frames.last_mut().unwrap().environment.code_owner = Some(owner);
            return Ok(());
        }

        let checkpoint = ArenaCheckpoint::new(program);
        let before_decl_len = program.declaration_sources.len();
        let before_refl_len = program.reflection_annotations.len();
        let before_alias_len = program.alias_definitions.len();
        let before_constr_len = program.constraint_origins.len();
        let before_base_mixin = program.base_mixin_declaration;
        let compiled = compile_unit_into_with_syntax(
            program,
            &resolved_ast,
            Some(Arc::clone(&resolved_ast)),
            cached_syntax,
            CompileOptions {
                fuse_constant_add: true,
            },
            power_assertions,
            Some(source.clone()),
            Some(ModuleIdentity {
                original_uri: resolved.original_uri.clone(),
                resolved_uri: resolved.resolved_uri.clone(),
            }),
            pure_standard_host,
        )
        .and_then(|unit| {
            if program
                .blocks
                .iter_from(checkpoint.blocks)
                .flatten()
                .any(|instruction| {
                    matches!(instruction, Instruction::Constant(value) if value.heap_id().is_some())
                })
            {
                Err(invalid("object constants cannot be stored in compiled code"))
            } else {
                Ok(unit)
            }
        });
        let unit = match compiled {
            Ok(unit) => unit,
            Err(mut error) => {
                checkpoint.rollback(program);
                if let Some(span) = error.span {
                    error.insert_frame(
                        0,
                        RuntimeFrame {
                            anonymous: false,
                            span,
                            source: Some(source),
                            member: PklString::default(),
                        },
                    );
                }
                return Err(error);
            }
        };
        if let Some(key) = &cache_key
            && program.units[unit.0 as usize].build.is_none()
            && program.units.next_id() == unit_id_usize + 1
            && program.base_mixin_declaration == before_base_mixin
        {
            store_compiled_unit(
                program,
                key,
                before_decl_len,
                before_refl_len,
                before_alias_len,
                before_constr_len,
                program.units[unit.0 as usize].clone(),
            );
        }
        program.units[unit.0 as usize].key = module_key;
        program.units[unit.0 as usize].delegate_key = delegate_key;
        let entry = program.units[unit.0 as usize].entry;
        let owner = self.heap.allocate_executable();
        self.code_arenas.push(arena.finish(program, owner, false));
        self.declaration_roots_dirty = true;
        self.heap.get_mut(receiver)?.environment.code_owner = Some(owner);
        self.state = State::Running;
        self.push_module_initializer(entry, Some(receiver));
        self.frames.last_mut().unwrap().environment.code_owner = Some(owner);
        Ok(())
    }

    pub(super) fn push_module_initializer(&mut self, entry: usize, receiver: Option<ObjectId>) {
        let mut frame = self.alloc_frame(entry, 0, self.stack.len(), Environment::default());
        frame.module_receiver = receiver;
        self.frames.push(frame);
    }

    /// Break cyclic module-import reads while a cached import's module
    /// initializer is already on the active call stack, matching Java
    /// `ModuleCache.doLoad` (`ModuleCache.java:180-185`) +
    /// `ImportNode.executeGeneric` (`ImportNode.java:57`).
    pub(super) fn active_cached_import_receiver(
        &self,
        receiver: ObjectId,
        owner: ObjectId,
        name: &PklString,
    ) -> Option<ObjectId> {
        self.frames.iter().rev().find_map(|frame| {
            let force = frame.force.as_ref()?;
            if force.entry
                || force.method
                || force.receiver != receiver
                || force.owner != owner
                || &force.name != name
            {
                return None;
            }
            let run = frame.module_load.as_ref()?;
            (run.purpose == ModulePurpose::Import
                && run.cached
                && run.initializing
                && run.phase == ModulePhase::Initializer)
                .then_some(run.receiver)
                .flatten()
        })
    }

    pub(super) fn step_module(&mut self) -> Result<(), RuntimeError> {
        let run = self
            .frames
            .last_mut()
            .and_then(|frame| frame.module_load.take())
            .ok_or_else(|| invalid("missing module continuation"))?;
        if !run.initializing || run.phase != ModulePhase::Initializer {
            return Err(invalid(
                "module continuation resumed before Host completion",
            ));
        }
        // The initializer already returned the actual receiver onto our stack.
        if self.stack.last().and_then(Value::heap_id) != run.receiver {
            return Err(invalid("module initializer result mismatch"));
        }
        let receiver = run
            .receiver
            .ok_or_else(|| invalid("missing initialized module"))?;
        if let Some(site) = run.site {
            self.heap
                .publish_imported_module(site.0, (site.1, site.2), receiver)?;
        }
        if run.purpose == ModulePurpose::Primary {
            self.public_primary = Some(receiver);
        }
        Ok(())
    }
}

fn stale_module() -> RuntimeError {
    RuntimeError::new(ErrorKind::Host, "stale or unexpected Host completion")
}

pub(super) fn imported_diagnostic(
    diagnostic: crate::diagnostic::Diagnostic,
    source: Arc<RuntimeSource>,
    member: PklString,
) -> RuntimeError {
    let mut error = RuntimeError::new(ErrorKind::Type, diagnostic.message.clone());
    error.span = Some(diagnostic.span);
    error.hint = diagnostic.hint().cloned();
    error.frames.push(RuntimeFrame {
        anonymous: false,
        span: diagnostic.span,
        source: Some(source.clone()),
        member: diagnostic
            .member
            .clone()
            .map_or_else(PklString::default, |suffix| {
                if suffix.encode_utf16().next() == Some(46) {
                    suffix.as_ref().clone()
                } else {
                    PklString::from("#").concat(&suffix)
                }
            }),
    });
    if let Some(foreign) = &diagnostic.provenance.foreign_exception {
        error.foreign_exception = Some(foreign.clone());
    }
    error.frontend = Some(Arc::new(crate::modules::ModuleDiagnostic {
        diagnostic,
        source,
        member,
    }));
    error
}

/// Compiler transactions never mutate an already published instruction or
/// declaration. Constraint origins are keyed by their newly appended blocks.
pub(super) struct ArenaCheckpoint {
    blocks: usize,
    metadata: usize,
    objects: usize,
    units: CodeStorage<ModuleUnit>,
    module_name: PklString,
    module_template: Option<usize>,
    entry: usize,
    initial_members: Vec<Member>,
    declaration: usize,
    type_recipes: usize,
    aliases: BTreeSet<usize>,
    base_mixin_declaration: Option<usize>,
}
impl ArenaCheckpoint {
    pub(super) fn new(program: &mut Program) -> Self {
        program.seal_code_segment();
        Self {
            blocks: program.blocks.next_id(),
            metadata: program.metadata.next_id(),
            objects: program.objects.next_id(),
            units: program.units.clone(),
            module_name: program.module_name.clone(),
            module_template: program.module_template,
            entry: program.entry,
            initial_members: program.initial_module_members.clone(),
            declaration: program.next_declaration,
            type_recipes: program.type_recipes.next_id(),
            aliases: program.alias_definitions.keys().copied().collect(),
            base_mixin_declaration: program.base_mixin_declaration,
        }
    }
    pub(super) fn rollback(self, program: &mut Program) {
        program.blocks.rollback(self.blocks);
        program.metadata.rollback(self.metadata);
        program.objects.rollback(self.objects);
        program.units = self.units;
        program.module_name = self.module_name;
        program.module_template = self.module_template;
        program.entry = self.entry;
        program.initial_module_members = self.initial_members;
        program.next_declaration = self.declaration;
        program.type_recipes.rollback(self.type_recipes);
        program.base_mixin_declaration = self.base_mixin_declaration;
        program
            .alias_definitions
            .retain(|id, _| self.aliases.contains(id));
        program
            .constraint_origins
            .retain(|block, _| *block < self.blocks);
        program
            .declaration_sources
            .retain(|block, _| *block < self.blocks);
        program
            .reflection_annotations
            .retain(|_, block| *block < self.blocks);
    }
}

fn validate_resolution(key: &ModuleKey, resolved: &ResolvedModule) -> Result<(), RuntimeError> {
    if resolved.original_uri != key.uri {
        return Err(invalid("module resolver changed original key"));
    }
    crate::host::resources::validate_module_key_uri(&resolved.resolved_uri)?;
    if key.kind == ModuleKeyKind::StandardLibrary
        && (resolved.resolved_uri != key.uri || resolved.source_uri != key.uri)
    {
        return Err(invalid(
            "Bundled module resolution must retain its pinned key and source identity.",
        ));
    }
    Ok(())
}
