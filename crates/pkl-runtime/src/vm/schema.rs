//! Host-drivable VM `ModuleSchema` extraction matching `ModuleInfo.getModuleSchema`,
//! `VmClass.export`, `VmTypeAlias.export`, `ClassProperty.export`, `ClassMethod.export`,
//! `FunctionNode.export`, `TypeNode.export`, and `VmModifier.export`.

use super::*;
use crate::evaluation::session::SourceSession;
use crate::pkl_binary_decode::{DecodedObject, DecodedValue, decode_lossless};
use crate::schema::{
    ClassId, Modifier, ModuleSchema, PClass, PClassInfo, PClassMethod, PClassProperty, PType,
    SchemaStep, SchemaStore, SourceLocation, TypeAlias, TypeAliasId, TypeAliasInfo, TypeParameter,
    Variance,
};
use crate::syntax::{DeclarationRecipe, DeclarationSource, Type, TypeKind};
use crate::value::RuntimeSource;
use crate::vm::classes::BuiltinClass;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, OnceLock};

#[derive(Clone, Debug)]
struct DeclInfo {
    owner: ObjectId,
    code: Option<usize>,
    source: Arc<RuntimeSource>,
    syntax: Arc<DeclarationSource>,
}

impl DeclInfo {
    fn parameters(&self) -> &[crate::syntax::TypeParameter] {
        match &self.syntax.recipe {
            DeclarationRecipe::Class {
                type_parameters, ..
            }
            | DeclarationRecipe::Method {
                type_parameters, ..
            }
            | DeclarationRecipe::TypeAlias {
                type_parameters, ..
            } => type_parameters,
            _ => &[],
        }
    }
}

#[derive(Clone, Debug)]
struct TypeCtx {
    owner: ObjectId,
    source: Arc<RuntimeSource>,
    parameters: Vec<crate::syntax::TypeParameter>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingPhase {
    None,
    ForceMembers,
    ExecuteBuiltinAnnotations,
    EncodeAnnotations,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct MemberForceKey {
    owner: ObjectId,
    name: PklString,
    local: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BuiltinAnnKey {
    owner: ObjectId,
    source_uri: PklString,
    header_span: Span,
}

pub(crate) struct VmSchemaExtractor {
    root_module: ObjectId,
    phase: PendingPhase,
    pending_builtin_anns: Vec<BuiltinAnnKey>,
    pending_ann_ids: Vec<ObjectId>,
    decoded_annotations: BTreeMap<ObjectId, DecodedObject>,
    /// Tracks members we already forced to avoid infinite loops if a member does not populate
    /// an expected cache slot.
    attempted_members: BTreeSet<MemberForceKey>,
    attempted_stdlibs: BTreeSet<PklString>,
}

impl VmSchemaExtractor {
    pub(crate) fn new(module: Value) -> Result<Self, RuntimeError> {
        let Some(id) = module.heap_id() else {
            return Err(invalid("evaluateSchema expected a module object"));
        };
        Ok(Self {
            root_module: id,
            phase: PendingPhase::None,
            pending_builtin_anns: Vec::new(),
            pending_ann_ids: Vec::new(),
            decoded_annotations: BTreeMap::new(),
            attempted_members: BTreeSet::new(),
            attempted_stdlibs: BTreeSet::new(),
        })
    }

    pub(crate) fn step(
        &mut self,
        session: &mut SourceSession,
        last_result: Option<Value>,
        now_ms: u64,
        timeout_ms: Option<u64>,
    ) -> Result<SchemaStep, RuntimeError> {
        let now_ms = now_ms.max(session.host_session().last_clock());
        // Process result from the previous pending VM block, if any.
        match self.phase {
            PendingPhase::None => {}
            PendingPhase::ForceMembers => {
                self.phase = PendingPhase::None;
                let _ = last_result;
            }
            PendingPhase::ExecuteBuiltinAnnotations => {
                self.phase = PendingPhase::None;
                if let Some(val) = last_result {
                    self.record_builtin_annotations(session.vm_mut(), val)?;
                }
            }
            PendingPhase::EncodeAnnotations => {
                self.phase = PendingPhase::None;
                if let Some(val) = last_result {
                    self.record_encoded_annotations(session.vm(), val)?;
                }
            }
        }

        // Phase 1: Iteratively force all reachable stdlib singletons, classes, type aliases,
        // and imports referenced in types until fixed point.
        let (needed_stdlibs, needed_members) = {
            let vm = session.vm();
            self.discover_uncached_dependencies(vm)?
        };
        if !needed_stdlibs.is_empty() || !needed_members.is_empty() {
            for uri in &needed_stdlibs {
                self.attempted_stdlibs.insert(uri.clone());
            }
            for req in &needed_members {
                self.attempted_members.insert(req.clone());
            }
            let (entry, inputs) =
                build_force_members_block(session.vm_mut(), &needed_stdlibs, &needed_members)?;
            session
                .host_session_mut()
                .start(entry, inputs, now_ms, timeout_ms)?;
            self.phase = PendingPhase::ForceMembers;
            return Ok(SchemaStep::RunPending);
        }

        // Phase 2: Execute any uncached builtin reflection_annotations collectors.
        let needed_builtin_anns = {
            let vm = session.vm();
            self.discover_uncached_builtin_annotations(vm)?
        };
        if !needed_builtin_anns.is_empty() {
            self.pending_builtin_anns = needed_builtin_anns;
            let (entry, inputs) =
                build_builtin_annotations_block(session.vm_mut(), &self.pending_builtin_anns)?;
            session
                .host_session_mut()
                .start(entry, inputs, now_ms, timeout_ms)?;
            self.phase = PendingPhase::ExecuteBuiltinAnnotations;
            return Ok(SchemaStep::RunPending);
        }

        // Phase 3: Force and pkl-binary encode all reachable annotation objects.
        let needed_ann_ids = {
            let vm = session.vm();
            self.discover_unencoded_annotations(vm)?
        };
        if !needed_ann_ids.is_empty() {
            self.pending_ann_ids = needed_ann_ids;
            let (entry, inputs) =
                build_encode_annotations_block(session.vm_mut(), &self.pending_ann_ids)?;
            session
                .host_session_mut()
                .start(entry, inputs, now_ms, timeout_ms)?;
            self.phase = PendingPhase::EncodeAnnotations;
            return Ok(SchemaStep::RunPending);
        }

        // Phase 4: Build complete ModuleSchema purely in memory.
        let schema = {
            let vm = session.vm();
            let builder = SchemaBuilder {
                vm,
                decoded_annotations: &self.decoded_annotations,
                store: SchemaStore::default(),
                class_ids: BTreeMap::new(),
                alias_ids: BTreeMap::new(),
                source_indices: BTreeMap::new(),
            };
            builder.export_module(self.root_module)?
        };
        Ok(SchemaStep::Done(schema))
    }

    fn record_builtin_annotations(
        &mut self,
        vm: &mut Vm,
        list_val: Value,
    ) -> Result<(), RuntimeError> {
        let collectors = extract_list_values(vm, list_val)?;
        let keys = std::mem::take(&mut self.pending_builtin_anns);
        for (key, collector_val) in keys.into_iter().zip(collectors) {
            let values = if let Some(cid) = collector_val.heap_id() {
                vm.heap.get(cid)?.annotations.clone()
            } else {
                Vec::new()
            };
            vm.heap
                .get_mut(key.owner)?
                .mirror_annotations
                .insert(key.header_span, values);
        }
        Ok(())
    }

    fn record_encoded_annotations(&mut self, vm: &Vm, list_val: Value) -> Result<(), RuntimeError> {
        let encoded_items = extract_list_values(vm, list_val)?;
        let ann_ids = std::mem::take(&mut self.pending_ann_ids);
        for (ann_id, bytes_val) in ann_ids.into_iter().zip(encoded_items) {
            let Value::Bytes(bytes) = bytes_val else {
                return Err(invalid("expected pkl-binary bytes for exported annotation"));
            };
            let decoded = decode_lossless(&bytes)
                .map_err(|err| invalid(&format!("failed to decode annotation: {err}")))?;
            let DecodedValue::Object(obj) = decoded else {
                return Err(invalid("expected DecodedValue::Object for annotation"));
            };
            self.decoded_annotations.insert(ann_id, obj);
        }
        Ok(())
    }

    fn discover_uncached_dependencies(
        &self,
        vm: &Vm,
    ) -> Result<(Vec<PklString>, Vec<MemberForceKey>), RuntimeError> {
        let mut needed_stdlibs: BTreeSet<PklString> = BTreeSet::new();
        let mut needed_members: BTreeSet<MemberForceKey> = BTreeSet::new();

        // Always ensure pkl:base is loaded (needed for builtin type resolution & superclasses).
        let base_uri: PklString = "pkl:base".into();
        if !matches!(
            vm.modules_stdlib.get("pkl:base"),
            Some(modules::CachedModule::Value(_))
        ) && !self.attempted_stdlibs.contains(&base_uri)
        {
            needed_stdlibs.insert(base_uri);
        }

        let mut visited_modules: BTreeSet<ObjectId> = BTreeSet::new();
        let mut visited_classes: BTreeSet<ObjectId> = BTreeSet::new();
        let mut visited_aliases: BTreeSet<ObjectId> = BTreeSet::new();
        let mut module_queue: Vec<ObjectId> = vec![self.root_module];
        let mut class_queue: Vec<ObjectId> = Vec::new();
        let mut alias_queue: Vec<ObjectId> = Vec::new();

        while !module_queue.is_empty() || !class_queue.is_empty() || !alias_queue.is_empty() {
            while let Some(mod_id) = module_queue.pop() {
                if !visited_modules.insert(mod_id) {
                    continue;
                }
                let obj = vm.heap.get(mod_id)?;
                if let Some(parent) = obj.parent
                    && vm.heap.get(parent).is_ok_and(|p| p.is_module)
                {
                    module_queue.push(parent);
                }
                if let Some(mod_cls) = vm.heap.object_class(mod_id)? {
                    class_queue.push(mod_cls);
                }
                let Some(unit) = obj
                    .module_unit
                    .and_then(|u| vm.program.units.get(u.0 as usize))
                else {
                    continue;
                };
                if unit.mode == modules::ModuleMode::Amends {
                    continue;
                }
                let Some(syntax) = unit.syntax.as_deref() else {
                    continue;
                };
                for member in &syntax.members {
                    if member.local {
                        continue;
                    }
                    let is_cls_or_alias = matches!(
                        member.expr.kind,
                        crate::syntax::ExprKind::Class { .. }
                            | crate::syntax::ExprKind::TypeAlias { .. }
                    );
                    if !is_cls_or_alias {
                        continue;
                    }
                    if let Some(val) = cached_member_value(vm, mod_id, &member.name, false)? {
                        match val {
                            Value::Class(cid) => class_queue.push(cid),
                            Value::TypeAlias(tid) => alias_queue.push(tid),
                            _ => {}
                        }
                    } else {
                        let req = MemberForceKey {
                            owner: mod_id,
                            name: member.name.clone(),
                            local: false,
                        };
                        if !self.attempted_members.contains(&req) {
                            needed_members.insert(req);
                        }
                    }
                }
            }

            while let Some(class_id) = class_queue.pop() {
                if !visited_classes.insert(class_id) {
                    continue;
                }
                let rec = vm.heap.get_class(class_id)?;
                if let Some(sup) = rec.superclass {
                    class_queue.push(sup);
                }
                let prototype = rec.prototype;
                let proto_obj = vm.heap.get(prototype)?;
                if !proto_obj.is_module {
                    // Enclosing module
                    if let Some(enc_mod) = find_enclosing_module_if_loaded(
                        vm,
                        prototype,
                        &class_origin_uri(vm, class_id)?,
                    ) {
                        if let Some(enc_cls) = vm.heap.object_class(enc_mod)? {
                            class_queue.push(enc_cls);
                        }
                    } else if let Some(uri) = class_origin_uri(vm, class_id)?
                        && uri.starts_with("pkl:")
                        && !self.attempted_stdlibs.contains(&uri)
                    {
                        needed_stdlibs.insert(uri);
                    }

                    // Supertype annotation
                    if let Ok(decl) = schema_class_declaration(vm, class_id)
                        && let DeclarationRecipe::Class {
                            superclass: Some(ref ty),
                            ..
                        } = decl.syntax.recipe
                    {
                        let ctx = schema_type_context(vm, &decl)?;
                        self.collect_type_deps(
                            vm,
                            ty,
                            &ctx,
                            &mut needed_stdlibs,
                            &mut needed_members,
                            &mut class_queue,
                            &mut alias_queue,
                        )?;
                    }
                }

                // Own properties and methods
                let props = schema_class_members(vm, class_id, false)?;
                for decl in props {
                    if let DeclarationRecipe::Property {
                        ty: Some(ref ty), ..
                    } = decl.syntax.recipe
                    {
                        let ctx = schema_type_context(vm, &decl)?;
                        self.collect_type_deps(
                            vm,
                            ty,
                            &ctx,
                            &mut needed_stdlibs,
                            &mut needed_members,
                            &mut class_queue,
                            &mut alias_queue,
                        )?;
                    }
                }
                let methods = schema_class_members(vm, class_id, true)?;
                for decl in methods {
                    if let DeclarationRecipe::Method {
                        ref parameters,
                        ref return_type,
                        ..
                    } = decl.syntax.recipe
                    {
                        let ctx = schema_type_context(vm, &decl)?;
                        for param in parameters {
                            if let Some(ref ty) = param.ty {
                                self.collect_type_deps(
                                    vm,
                                    ty,
                                    &ctx,
                                    &mut needed_stdlibs,
                                    &mut needed_members,
                                    &mut class_queue,
                                    &mut alias_queue,
                                )?;
                            }
                        }
                        if let Some(ty) = return_type {
                            self.collect_type_deps(
                                vm,
                                ty,
                                &ctx,
                                &mut needed_stdlibs,
                                &mut needed_members,
                                &mut class_queue,
                                &mut alias_queue,
                            )?;
                        }
                    }
                }
            }

            while let Some(alias_id) = alias_queue.pop() {
                if !visited_aliases.insert(alias_id) {
                    continue;
                }
                let decl = schema_alias_declaration(vm, alias_id)?;
                if let Some(enc_mod) =
                    find_enclosing_module_if_loaded(vm, decl.owner, &Some(decl.source.uri.clone()))
                {
                    if let Some(enc_cls) = vm.heap.object_class(enc_mod)? {
                        class_queue.push(enc_cls);
                    }
                } else if decl.source.uri.starts_with("pkl:")
                    && !self.attempted_stdlibs.contains(&decl.source.uri)
                {
                    needed_stdlibs.insert(decl.source.uri.clone());
                }
                if let DeclarationRecipe::TypeAlias { ref referent, .. } = decl.syntax.recipe {
                    let ctx = schema_type_context(vm, &decl)?;
                    self.collect_type_deps(
                        vm,
                        referent,
                        &ctx,
                        &mut needed_stdlibs,
                        &mut needed_members,
                        &mut class_queue,
                        &mut alias_queue,
                    )?;
                }
            }
        }

        Ok((
            needed_stdlibs.into_iter().collect(),
            needed_members.into_iter().collect(),
        ))
    }

    #[allow(clippy::too_many_arguments)]
    fn collect_type_deps(
        &self,
        vm: &Vm,
        ty: &Type,
        ctx: &TypeCtx,
        needed_stdlibs: &mut BTreeSet<PklString>,
        needed_members: &mut BTreeSet<MemberForceKey>,
        class_queue: &mut Vec<ObjectId>,
        alias_queue: &mut Vec<ObjectId>,
    ) -> Result<(), RuntimeError> {
        match &ty.kind {
            TypeKind::Unknown | TypeKind::Nothing | TypeKind::Module | TypeKind::String(_) => {}
            TypeKind::Nullable(inner) => {
                self.collect_type_deps(
                    vm,
                    inner,
                    ctx,
                    needed_stdlibs,
                    needed_members,
                    class_queue,
                    alias_queue,
                )?;
            }
            TypeKind::Union { types, .. } => {
                for t in types {
                    self.collect_type_deps(
                        vm,
                        t,
                        ctx,
                        needed_stdlibs,
                        needed_members,
                        class_queue,
                        alias_queue,
                    )?;
                }
            }
            TypeKind::Function {
                parameters,
                return_type,
            } => {
                for t in parameters {
                    self.collect_type_deps(
                        vm,
                        t,
                        ctx,
                        needed_stdlibs,
                        needed_members,
                        class_queue,
                        alias_queue,
                    )?;
                }
                self.collect_type_deps(
                    vm,
                    return_type,
                    ctx,
                    needed_stdlibs,
                    needed_members,
                    class_queue,
                    alias_queue,
                )?;
            }
            TypeKind::Constrained { base, .. } => {
                self.collect_type_deps(
                    vm,
                    base,
                    ctx,
                    needed_stdlibs,
                    needed_members,
                    class_queue,
                    alias_queue,
                )?;
            }
            TypeKind::Named {
                path,
                name_spans,
                arguments,
            } => {
                if path.len() == 1 && ctx.parameters.iter().any(|p| p.name == path[0]) {
                    return Ok(());
                }
                let Some(scope_mod) = resolve_scope_module_if_loaded(vm, ctx)? else {
                    if ctx.source.uri.starts_with("pkl:")
                        && !self.attempted_stdlibs.contains(&ctx.source.uri)
                    {
                        needed_stdlibs.insert(ctx.source.uri.clone());
                    }
                    return Ok(());
                };

                let mut is_erased_class = false;
                if path.len() == 1 {
                    let name = &path[0];
                    if let Some((owner, local)) =
                        locate_simple_type_owner(vm, scope_mod, &ctx.source.uri, name, ty.span)?
                    {
                        if let Some(val) = cached_member_value(vm, owner, name, local)? {
                            match val {
                                Value::Class(cid) => {
                                    is_erased_class =
                                        vm.canonical_type_class(cid)? == Some(BuiltinClass::Class);
                                    class_queue.push(cid);
                                }
                                Value::TypeAlias(tid) => alias_queue.push(tid),
                                Value::Object(mod_id) => {
                                    if let Ok(mc) = vm.declared_module_class(mod_id, ty.span) {
                                        class_queue.push(mc);
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            let req = MemberForceKey {
                                owner,
                                name: name.clone(),
                                local,
                            };
                            if !self.attempted_members.contains(&req) {
                                needed_members.insert(req);
                            }
                        }
                    }
                } else if path.len() == 2 && name_spans.len() == 2 {
                    let import_name = &path[0];
                    let type_name = &path[1];
                    let import_member =
                        vm.declared_type_import(scope_mod, import_name, name_spans[0])?;
                    if let Some(Value::Object(imported_mod)) =
                        cached_member_value(vm, scope_mod, import_name, import_member.local)?
                    {
                        if let Some((owner, local)) = locate_qualified_type_owner(
                            vm,
                            imported_mod,
                            type_name,
                            Span::new(name_spans[0].start, name_spans[1].end),
                        )? {
                            if let Some(val) = cached_member_value(vm, owner, type_name, local)? {
                                match val {
                                    Value::Class(cid) => {
                                        is_erased_class = vm.canonical_type_class(cid)?
                                            == Some(BuiltinClass::Class);
                                        class_queue.push(cid);
                                    }
                                    Value::TypeAlias(tid) => alias_queue.push(tid),
                                    _ => {}
                                }
                            } else {
                                let req = MemberForceKey {
                                    owner,
                                    name: type_name.clone(),
                                    local,
                                };
                                if !self.attempted_members.contains(&req) {
                                    needed_members.insert(req);
                                }
                            }
                        }
                    } else {
                        let req = MemberForceKey {
                            owner: scope_mod,
                            name: import_name.clone(),
                            local: import_member.local,
                        };
                        if !self.attempted_members.contains(&req) {
                            needed_members.insert(req);
                        }
                    }
                }

                if !is_erased_class {
                    for arg in arguments {
                        self.collect_type_deps(
                            vm,
                            arg,
                            ctx,
                            needed_stdlibs,
                            needed_members,
                            class_queue,
                            alias_queue,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn discover_uncached_builtin_annotations(
        &self,
        vm: &Vm,
    ) -> Result<Vec<BuiltinAnnKey>, RuntimeError> {
        let reachable = collect_all_reachable(vm, self.root_module)?;
        let mut needed: BTreeSet<BuiltinAnnKey> = BTreeSet::new();

        for class_id in &reachable.classes {
            let proto = vm.heap.get_class(*class_id)?.prototype;
            if !vm.heap.get(proto)?.is_module
                && let Ok(decl) = schema_class_declaration(vm, *class_id)
            {
                check_builtin_decl_ann(vm, &decl, &mut needed)?;
            }
            for decl in schema_class_members(vm, *class_id, false)? {
                check_builtin_decl_ann(vm, &decl, &mut needed)?;
            }
            for decl in schema_class_members(vm, *class_id, true)? {
                check_builtin_decl_ann(vm, &decl, &mut needed)?;
            }
        }
        for alias_id in &reachable.aliases {
            let decl = schema_alias_declaration(vm, *alias_id)?;
            check_builtin_decl_ann(vm, &decl, &mut needed)?;
        }

        Ok(needed.into_iter().collect())
    }

    fn discover_unencoded_annotations(&self, vm: &Vm) -> Result<Vec<ObjectId>, RuntimeError> {
        let reachable = collect_all_reachable(vm, self.root_module)?;
        let mut needed: BTreeSet<ObjectId> = BTreeSet::new();

        let mut push_values = |vals: &[Value]| {
            for val in vals {
                if let Some(id) = val.heap_id()
                    && !self.decoded_annotations.contains_key(&id)
                {
                    needed.insert(id);
                }
            }
        };

        for mod_id in &reachable.modules {
            let obj = vm.heap.get(*mod_id)?;
            push_values(&obj.annotations);
        }
        for class_id in &reachable.classes {
            let proto = vm.heap.get_class(*class_id)?.prototype;
            let obj = vm.heap.get(proto)?;
            if obj.is_module {
                push_values(&obj.annotations);
            } else if let Ok(decl) = schema_class_declaration(vm, *class_id) {
                let anns = existing_decl_annotations(vm, &decl)?;
                push_values(&anns);
            }
            for decl in schema_class_members(vm, *class_id, false)? {
                let anns = existing_decl_annotations(vm, &decl)?;
                push_values(&anns);
            }
            for decl in schema_class_members(vm, *class_id, true)? {
                let anns = existing_decl_annotations(vm, &decl)?;
                push_values(&anns);
            }
        }
        for alias_id in &reachable.aliases {
            let decl = schema_alias_declaration(vm, *alias_id)?;
            let anns = existing_decl_annotations(vm, &decl)?;
            push_values(&anns);
        }

        Ok(needed.into_iter().collect())
    }
}

fn check_builtin_decl_ann(
    vm: &Vm,
    decl: &DeclInfo,
    needed: &mut BTreeSet<BuiltinAnnKey>,
) -> Result<(), RuntimeError> {
    let obj = vm.heap.get(decl.owner)?;
    if obj.builtin_type.is_none() && decl.code.is_some() {
        return Ok(());
    }
    if obj
        .mirror_annotations
        .contains_key(&decl.syntax.header_span)
    {
        return Ok(());
    }
    if vm
        .program
        .reflection_annotations
        .contains_key(&(decl.source.uri.clone(), decl.syntax.header_span))
    {
        needed.insert(BuiltinAnnKey {
            owner: decl.owner,
            source_uri: decl.source.uri.clone(),
            header_span: decl.syntax.header_span,
        });
    }
    Ok(())
}

fn existing_decl_annotations(vm: &Vm, decl: &DeclInfo) -> Result<Vec<Value>, RuntimeError> {
    let obj = vm.heap.get(decl.owner)?;
    if obj.builtin_type.is_none()
        && let Some(code) = decl.code
    {
        if matches!(
            decl.syntax.recipe,
            DeclarationRecipe::Class { .. } | DeclarationRecipe::TypeAlias { .. }
        ) {
            return Ok(obj.annotations.clone());
        }
        return Ok(obj
            .member_annotations
            .get(&code)
            .cloned()
            .unwrap_or_default());
    }
    if let Some(vals) = obj.mirror_annotations.get(&decl.syntax.header_span) {
        return Ok(vals.clone());
    }
    if matches!(
        decl.syntax.recipe,
        DeclarationRecipe::Class { .. } | DeclarationRecipe::TypeAlias { .. }
    ) {
        return Ok(obj.annotations.clone());
    }
    Ok(Vec::new())
}

struct ReachableSet {
    modules: BTreeSet<ObjectId>,
    classes: BTreeSet<ObjectId>,
    aliases: BTreeSet<ObjectId>,
}

fn collect_all_reachable(vm: &Vm, root_module: ObjectId) -> Result<ReachableSet, RuntimeError> {
    let mut modules = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut aliases = BTreeSet::new();
    let mut mod_q = vec![root_module];
    let mut cls_q = Vec::new();
    let mut alias_q = Vec::new();

    while !mod_q.is_empty() || !cls_q.is_empty() || !alias_q.is_empty() {
        while let Some(mod_id) = mod_q.pop() {
            if !modules.insert(mod_id) {
                continue;
            }
            let obj = vm.heap.get(mod_id)?;
            if let Some(parent) = obj.parent
                && vm.heap.get(parent).is_ok_and(|p| p.is_module)
            {
                mod_q.push(parent);
            }
            if let Some(mc) = vm.heap.object_class(mod_id)? {
                cls_q.push(mc);
            }
            let Some(unit) = obj
                .module_unit
                .and_then(|u| vm.program.units.get(u.0 as usize))
            else {
                continue;
            };
            if unit.mode == modules::ModuleMode::Amends {
                continue;
            }
            let Some(syntax) = unit.syntax.as_deref() else {
                continue;
            };
            for member in &syntax.members {
                if member.local {
                    continue;
                }
                if matches!(
                    member.expr.kind,
                    crate::syntax::ExprKind::Class { .. }
                        | crate::syntax::ExprKind::TypeAlias { .. }
                ) && let Some(val) = cached_member_value(vm, mod_id, &member.name, false)?
                {
                    match val {
                        Value::Class(cid) => cls_q.push(cid),
                        Value::TypeAlias(tid) => alias_q.push(tid),
                        _ => {}
                    }
                }
            }
        }
        while let Some(cid) = cls_q.pop() {
            if !classes.insert(cid) {
                continue;
            }
            let rec = vm.heap.get_class(cid)?;
            if let Some(sup) = rec.superclass {
                cls_q.push(sup);
            }
            let proto = rec.prototype;
            if !vm.heap.get(proto)?.is_module {
                if let Some(enc) =
                    find_enclosing_module_if_loaded(vm, proto, &class_origin_uri(vm, cid)?)
                    && let Some(mc) = vm.heap.object_class(enc)?
                {
                    cls_q.push(mc);
                }
                if let Ok(decl) = schema_class_declaration(vm, cid)
                    && let DeclarationRecipe::Class {
                        superclass: Some(ref ty),
                        ..
                    } = decl.syntax.recipe
                {
                    let ctx = schema_type_context(vm, &decl)?;
                    collect_cached_type_targets(vm, ty, &ctx, &mut cls_q, &mut alias_q)?;
                }
            }
            for decl in schema_class_members(vm, cid, false)? {
                if let DeclarationRecipe::Property {
                    ty: Some(ref ty), ..
                } = decl.syntax.recipe
                {
                    let ctx = schema_type_context(vm, &decl)?;
                    collect_cached_type_targets(vm, ty, &ctx, &mut cls_q, &mut alias_q)?;
                }
            }
            for decl in schema_class_members(vm, cid, true)? {
                if let DeclarationRecipe::Method {
                    ref parameters,
                    ref return_type,
                    ..
                } = decl.syntax.recipe
                {
                    let ctx = schema_type_context(vm, &decl)?;
                    for p in parameters {
                        if let Some(ref ty) = p.ty {
                            collect_cached_type_targets(vm, ty, &ctx, &mut cls_q, &mut alias_q)?;
                        }
                    }
                    if let Some(ty) = return_type {
                        collect_cached_type_targets(vm, ty, &ctx, &mut cls_q, &mut alias_q)?;
                    }
                }
            }
        }
        while let Some(tid) = alias_q.pop() {
            if !aliases.insert(tid) {
                continue;
            }
            let decl = schema_alias_declaration(vm, tid)?;
            if let Some(enc) =
                find_enclosing_module_if_loaded(vm, decl.owner, &Some(decl.source.uri.clone()))
                && let Some(mc) = vm.heap.object_class(enc)?
            {
                cls_q.push(mc);
            }
            if let DeclarationRecipe::TypeAlias { ref referent, .. } = decl.syntax.recipe {
                let ctx = schema_type_context(vm, &decl)?;
                collect_cached_type_targets(vm, referent, &ctx, &mut cls_q, &mut alias_q)?;
            }
        }
    }

    Ok(ReachableSet {
        modules,
        classes,
        aliases,
    })
}

fn collect_cached_type_targets(
    vm: &Vm,
    ty: &Type,
    ctx: &TypeCtx,
    cls_q: &mut Vec<ObjectId>,
    alias_q: &mut Vec<ObjectId>,
) -> Result<(), RuntimeError> {
    match &ty.kind {
        TypeKind::Unknown | TypeKind::Nothing | TypeKind::Module | TypeKind::String(_) => {}
        TypeKind::Nullable(inner) | TypeKind::Constrained { base: inner, .. } => {
            collect_cached_type_targets(vm, inner, ctx, cls_q, alias_q)?;
        }
        TypeKind::Union { types, .. } => {
            for t in types {
                collect_cached_type_targets(vm, t, ctx, cls_q, alias_q)?;
            }
        }
        TypeKind::Function {
            parameters,
            return_type,
        } => {
            for t in parameters {
                collect_cached_type_targets(vm, t, ctx, cls_q, alias_q)?;
            }
            collect_cached_type_targets(vm, return_type, ctx, cls_q, alias_q)?;
        }
        TypeKind::Named {
            path,
            name_spans,
            arguments,
        } => {
            if path.len() == 1 && ctx.parameters.iter().any(|p| p.name == path[0]) {
                return Ok(());
            }
            let mut is_erased_class = false;
            if let Some(val) = resolve_cached_named_type(vm, path, name_spans, ty.span, ctx)? {
                match val {
                    Value::Class(cid) => {
                        is_erased_class =
                            vm.canonical_type_class(cid)? == Some(BuiltinClass::Class);
                        cls_q.push(cid);
                    }
                    Value::TypeAlias(tid) => alias_q.push(tid),
                    Value::Object(mod_id) => {
                        if let Ok(mc) = vm.declared_module_class(mod_id, ty.span) {
                            cls_q.push(mc);
                        }
                    }
                    _ => {}
                }
            }
            if !is_erased_class {
                for arg in arguments {
                    collect_cached_type_targets(vm, arg, ctx, cls_q, alias_q)?;
                }
            }
        }
    }
    Ok(())
}

fn build_force_members_block(
    vm: &mut Vm,
    needed_stdlibs: &[PklString],
    needed_members: &[MemberForceKey],
) -> Result<(usize, Vec<Value>), RuntimeError> {
    let mut code = Vec::new();
    let mut inputs = Vec::new();

    for uri in needed_stdlibs {
        if uri == "pkl:base" {
            code.push(Instruction::LoadBaseModule);
            code.push(Instruction::Pop);
        } else {
            code.push(Instruction::ImportModule(modules::ImportSpec {
                authored: uri.clone(),
                span: Span::default(),
                unit: crate::modules::UnitId(u32::MAX),
                absolute_uri: uri.clone(),
                original_uri: uri.clone(),
            }));
            code.push(Instruction::Pop);
        }
    }

    // Pre-allocate 0-arity thunk functions for local member reads so they go through
    // exact `Instruction::GetProperty { owner_depth: 0, local: true }` force semantics.
    let mut local_thunks: Vec<ObjectId> = Vec::new();
    for req in needed_members {
        if req.local {
            let program = Arc::make_mut(&mut vm.program);
            let thunk_block = program.alloc_block(
                vec![
                    Instruction::GetProperty {
                        name: req.name.clone(),
                        owner_depth: 0,
                        local: true,
                    },
                    Instruction::Return,
                ],
                BlockMetadata::default(),
            )?;
            let mut environment = vm.heap.get(req.owner)?.environment.clone();
            environment.this_value = Some(Value::Object(req.owner));
            environment.scopes.insert(
                0,
                Scope {
                    owner: req.owner,
                    receiver: req.owner,
                },
            );
            let fn_id = vm.heap.allocate(Object {
                function: Some(Function {
                    block: thunk_block,
                    arity: 0,
                    method: false,
                    captured_this: None,
                }),
                environment,
                ..Object::default()
            });
            local_thunks.push(fn_id);
        }
    }

    let mut local_idx = 0usize;
    for req in needed_members {
        if req.local {
            let fn_id = local_thunks[local_idx];
            local_idx += 1;
            let idx = inputs.len();
            inputs.push(Value::Function(fn_id));
            code.push(Instruction::Input(idx));
            code.push(Instruction::Call {
                arity: 0,
                method: false,
            });
            code.push(Instruction::Pop);
        } else {
            let idx = inputs.len();
            inputs.push(Value::Object(req.owner));
            code.push(Instruction::Input(idx));
            code.push(Instruction::GetMember(req.name.clone()));
            code.push(Instruction::Pop);
        }
    }

    code.push(Instruction::Constant(Value::Null));
    code.push(Instruction::Return);

    let program = Arc::make_mut(&mut vm.program);
    let entry = program.alloc_block(code, BlockMetadata::default())?;
    Ok((entry, inputs))
}

fn build_builtin_annotations_block(
    vm: &mut Vm,
    keys: &[BuiltinAnnKey],
) -> Result<(usize, Vec<Value>), RuntimeError> {
    let mut code = Vec::new();
    let mut inputs = Vec::new();

    for key in keys {
        let Some(&block) = vm
            .program
            .reflection_annotations
            .get(&(key.source_uri.clone(), key.header_span))
        else {
            code.push(Instruction::Constant(Value::Null));
            continue;
        };
        let Some(modules::CachedModule::Value(module_id)) =
            vm.modules_stdlib.get(&key.source_uri).cloned()
        else {
            code.push(Instruction::Constant(Value::Null));
            continue;
        };
        let mut environment = vm.heap.get(module_id)?.environment.clone();
        environment.this_value = Some(Value::Object(module_id));
        environment.scopes.insert(
            0,
            Scope {
                owner: module_id,
                receiver: module_id,
            },
        );
        let fn_id = vm.heap.allocate(Object {
            function: Some(Function {
                block,
                arity: 0,
                method: false,
                captured_this: None,
            }),
            environment,
            ..Object::default()
        });
        let idx = inputs.len();
        inputs.push(Value::Function(fn_id));
        code.push(Instruction::Input(idx));
        code.push(Instruction::Call {
            arity: 0,
            method: false,
        });
    }

    code.push(Instruction::MakeList { length: keys.len() });
    code.push(Instruction::Return);

    let program = Arc::make_mut(&mut vm.program);
    let entry = program.alloc_block(code, BlockMetadata::default())?;
    Ok((entry, inputs))
}

fn build_encode_annotations_block(
    vm: &mut Vm,
    ann_ids: &[ObjectId],
) -> Result<(usize, Vec<Value>), RuntimeError> {
    let mut code = Vec::new();
    let mut inputs = Vec::with_capacity(ann_ids.len());

    for &ann_id in ann_ids {
        let idx = inputs.len();
        inputs.push(Value::Object(ann_id));
        code.push(Instruction::Input(idx));
        code.push(Instruction::EncodePklBinary);
    }

    code.push(Instruction::MakeList {
        length: ann_ids.len(),
    });
    code.push(Instruction::Return);

    let program = Arc::make_mut(&mut vm.program);
    let entry = program.alloc_block(code, BlockMetadata::default())?;
    Ok((entry, inputs))
}

fn extract_list_values(vm: &Vm, list_val: Value) -> Result<Vec<Value>, RuntimeError> {
    let id = list_val
        .heap_id()
        .ok_or_else(|| invalid("expected list result from schema preparation"))?;
    let mut out = Vec::new();
    for member in collections::ordered_members(&vm.heap, id)? {
        let val = collections::cached_render_value(&vm.heap, id, &member)?
            .ok_or_else(|| invalid("strict schema preparation list element missing"))?;
        out.push(val);
    }
    Ok(out)
}

fn cached_member_value(
    vm: &Vm,
    owner: ObjectId,
    name: &PklString,
    local: bool,
) -> Result<Option<Value>, RuntimeError> {
    let key = CacheKey {
        entry: false,
        owner: if local { Some(owner) } else { None },
        name: name.clone(),
        method: false,
    };
    Ok(match vm.heap.get(owner)?.cache.get(&key) {
        Some(LazyState::Completed(v)) => Some(v.clone()),
        _ => None,
    })
}

fn pinned_module(
    uri: &PklString,
) -> Result<(&'static crate::syntax::Module, Arc<RuntimeSource>), RuntimeError> {
    static BASE: OnceLock<crate::syntax::Module> = OnceLock::new();
    static REF: OnceLock<crate::syntax::Module> = OnceLock::new();
    static REFLECT: OnceLock<crate::syntax::Module> = OnceLock::new();
    let (cell, source) = match uri.as_str() {
        Some("pkl:base") => (&BASE, crate::stdlib_sources::base()),
        Some("pkl:ref") => (&REF, crate::stdlib_sources::reference()),
        Some("pkl:reflect") => (&REFLECT, crate::stdlib_sources::reflect()),
        _ => return Err(invalid("unknown builtin declaration source")),
    };
    Ok((
        cell.get_or_init(|| {
            crate::parser::parse_module(&source.text).expect("pinned metadata syntax")
        }),
        source,
    ))
}

fn declaration_from_code(vm: &Vm, owner: ObjectId, code: usize) -> Result<DeclInfo, RuntimeError> {
    let syntax = vm
        .program
        .declaration_sources
        .get(&code)
        .ok_or_else(|| invalid("missing original declaration recipe"))?
        .clone();
    let source = vm
        .program
        .metadata
        .get(code)
        .and_then(|metadata| {
            metadata.source.clone().or_else(|| {
                metadata
                    .unit
                    .and_then(|unit| vm.program.units.get(unit.0 as usize))
                    .and_then(|unit| unit.source.clone())
            })
        })
        .ok_or_else(|| invalid("declaration source missing"))?;
    Ok(DeclInfo {
        owner,
        code: Some(code),
        source,
        syntax: Arc::new(syntax),
    })
}

fn class_origin_uri(vm: &Vm, class_id: ObjectId) -> Result<Option<PklString>, RuntimeError> {
    let def = vm.heap.class_definition(class_id)?;
    Ok(def.origin_uri.clone())
}

fn find_enclosing_module_if_loaded(
    vm: &Vm,
    owner: ObjectId,
    origin_uri: &Option<PklString>,
) -> Option<ObjectId> {
    let obj = vm.heap.get(owner).ok()?;
    if let Some(module) = std::iter::once(owner)
        .chain(obj.environment.scopes.iter().map(|s| s.owner))
        .find(|id| vm.heap.get(*id).is_ok_and(|o| o.is_module))
    {
        return Some(module);
    }
    let uri = origin_uri
        .as_ref()
        .and_then(|u| u.as_str())
        .unwrap_or("pkl:base");
    if let Some(modules::CachedModule::Value(mod_id)) = vm.modules_stdlib.get(uri) {
        return Some(*mod_id);
    }
    None
}

fn schema_class_declaration(vm: &Vm, class_id: ObjectId) -> Result<DeclInfo, RuntimeError> {
    let prototype = vm.heap.get_class(class_id)?.prototype;
    let obj = vm.heap.get(prototype)?;
    if let Some(code) = obj.declaration_code {
        return declaration_from_code(vm, prototype, code);
    }
    let def = vm.heap.class_definition(class_id)?;
    let uri = def.origin_uri.clone().unwrap_or_else(|| "pkl:base".into());
    let (parsed, source) = pinned_module(&uri)?;
    let member = parsed
        .members
        .iter()
        .find(|m| {
            m.name == def.name && matches!(m.expr.kind, crate::syntax::ExprKind::Class { .. })
        })
        .ok_or_else(|| invalid("builtin class metadata not found"))?;
    Ok(DeclInfo {
        owner: prototype,
        code: None,
        source,
        syntax: Arc::new(member.declaration_source()),
    })
}

fn schema_alias_declaration(vm: &Vm, alias_id: ObjectId) -> Result<DeclInfo, RuntimeError> {
    let obj = vm.heap.get(alias_id)?;
    if let Some(code) = obj.declaration_code {
        return declaration_from_code(vm, alias_id, code);
    }
    let qualified = obj
        .type_alias
        .as_ref()
        .ok_or_else(|| invalid("alias declaration name"))?;
    let qualified_text = crate::source::SourceText::from(qualified);
    let name = qualified_text
        .entire()
        .rsplit_once("#")
        .map_or(qualified.clone(), |(_, n)| n.into());
    let base_uri: PklString = "pkl:base".into();
    let (parsed, source) = pinned_module(&base_uri)?;
    let member = parsed
        .members
        .iter()
        .find(|m| {
            m.name == name && matches!(m.expr.kind, crate::syntax::ExprKind::TypeAlias { .. })
        })
        .ok_or_else(|| invalid("builtin alias metadata not found"))?;
    Ok(DeclInfo {
        owner: alias_id,
        code: None,
        source,
        syntax: Arc::new(member.declaration_source()),
    })
}

/// Returns a class's OWN property or method declarations matching `VmClass.export` +
/// `VmClass.isClassPropertyDefinition` (untyped property overrides of inherited properties
/// remain owned by the superclass, not re-declared on the subclass).
fn schema_class_members(
    vm: &Vm,
    class_id: ObjectId,
    methods: bool,
) -> Result<Vec<DeclInfo>, RuntimeError> {
    let rec = vm.heap.get_class(class_id)?;
    let owner = rec.prototype;
    let obj = vm.heap.get(owner)?;

    // Build set of property names defined on any superclass so we can filter untyped overrides.
    let super_props: BTreeSet<PklString> = if !methods {
        let mut names = BTreeSet::new();
        let mut cur = rec.superclass;
        while let Some(sup_id) = cur {
            let sup_rec = vm.heap.get_class(sup_id)?;
            for d in raw_class_members(vm, sup_id, false)? {
                names.insert(d.syntax.name.clone());
            }
            cur = sup_rec.superclass;
        }
        names
    } else {
        BTreeSet::new()
    };

    let raw = raw_class_members(vm, class_id, methods)?;
    let mut out = Vec::new();
    for decl in raw {
        if !methods
            && matches!(
                decl.syntax.recipe,
                DeclarationRecipe::Property { ty: None, .. }
            )
            && super_props.contains(&decl.syntax.name)
        {
            continue;
        }
        // `obj` might be a module amending another module; wait: in Pkl, an amending module
        // does not add class properties unless typed or new, which is already covered above.
        let _ = obj;
        out.push(decl);
    }
    Ok(out)
}

fn raw_class_members(
    vm: &Vm,
    class_id: ObjectId,
    methods: bool,
) -> Result<Vec<DeclInfo>, RuntimeError> {
    let owner = vm.heap.get_class(class_id)?.prototype;
    let obj = vm.heap.get(owner)?;
    if obj.builtin_type.is_some() {
        let def = vm.heap.class_definition(class_id)?;
        let uri = def.origin_uri.clone().unwrap_or_else(|| "pkl:base".into());
        let (parsed, source) = pinned_module(&uri)?;
        let member = parsed
            .members
            .iter()
            .find(|m| {
                m.name == def.name && matches!(m.expr.kind, crate::syntax::ExprKind::Class { .. })
            })
            .ok_or_else(|| invalid("builtin class members"))?;
        let crate::syntax::ExprKind::Class { members, .. } = &member.expr.kind else {
            unreachable!()
        };
        Ok(members
            .iter()
            .filter(|m| {
                !m.local
                    && !matches!(
                        m.expr.kind,
                        crate::syntax::ExprKind::Class { .. }
                            | crate::syntax::ExprKind::TypeAlias { .. }
                            | crate::syntax::ExprKind::Import { .. }
                    )
                    && matches!(
                        m.expr.kind,
                        crate::syntax::ExprKind::Function { method: true, .. }
                    ) == methods
            })
            .map(|m| DeclInfo {
                owner,
                code: obj
                    .members
                    .iter()
                    .find(|compiled| compiled.name == m.name && compiled.method == methods)
                    .map(|compiled| compiled.code),
                source: source.clone(),
                syntax: Arc::new(m.declaration_source()),
            })
            .collect())
    } else {
        obj.members
            .iter()
            .filter(|m| !m.local && m.method == methods && !m.import() && !m.definition())
            .map(|m| declaration_from_code(vm, owner, m.code))
            .collect()
    }
}

fn schema_type_context(vm: &Vm, decl: &DeclInfo) -> Result<TypeCtx, RuntimeError> {
    let mut parameters = Vec::new();
    if matches!(
        decl.syntax.recipe,
        DeclarationRecipe::Property { .. } | DeclarationRecipe::Method { .. }
    ) && !vm.heap.get(decl.owner)?.is_module
        && let Some(class_id) = vm.heap.object_class(decl.owner)?
    {
        parameters.extend(
            schema_class_declaration(vm, class_id)?
                .parameters()
                .iter()
                .cloned(),
        );
    }
    for param in decl.parameters() {
        if let Some(idx) = parameters.iter().position(|p| p.name == param.name) {
            parameters[idx] = param.clone();
        } else {
            parameters.push(param.clone());
        }
    }
    Ok(TypeCtx {
        owner: decl.owner,
        source: decl.source.clone(),
        parameters,
    })
}

fn resolve_scope_module_if_loaded(
    vm: &Vm,
    ctx: &TypeCtx,
) -> Result<Option<ObjectId>, RuntimeError> {
    let obj = vm.heap.get(ctx.owner)?;
    if obj.is_module {
        return Ok(Some(ctx.owner));
    }
    if obj.environment.scopes.is_empty() && ctx.source.uri.starts_with("pkl:") {
        return Ok(match vm.modules_stdlib.get(&ctx.source.uri) {
            Some(modules::CachedModule::Value(id)) => Some(*id),
            _ => None,
        });
    }
    Ok(Some(vm.declaring_module(&obj.environment, false)?))
}

fn locate_simple_type_owner(
    vm: &Vm,
    module: ObjectId,
    source_uri: &PklString,
    name: &PklString,
    span: Span,
) -> Result<Option<(ObjectId, bool)>, RuntimeError> {
    if let Some(m) = vm.declared_type_member(module, name, true, true, span)? {
        return Ok(Some((module, m.local)));
    }
    if let Some(m) = vm.declared_type_member(module, name, false, true, span)? {
        return Ok(Some((module, m.local)));
    }
    if source_uri != "pkl:base"
        && let Some(modules::CachedModule::Value(base)) = vm.modules_stdlib.get("pkl:base")
        && let Some(m) = vm.declared_type_member(*base, name, false, true, span)?
    {
        return Ok(Some((*base, m.local)));
    }
    let mut cur = vm.heap.get(module)?.parent;
    while let Some(owner) = cur {
        if let Some(m) = vm.declared_type_member(owner, name, false, true, span)? {
            return Ok(Some((owner, m.local)));
        }
        cur = vm.heap.get(owner)?.parent;
    }
    Ok(None)
}

fn locate_qualified_type_owner(
    vm: &Vm,
    imported_mod: ObjectId,
    name: &PklString,
    span: Span,
) -> Result<Option<(ObjectId, bool)>, RuntimeError> {
    let mut cur = Some(imported_mod);
    while let Some(owner) = cur {
        if let Some(m) = vm.declared_type_member(owner, name, false, false, span)? {
            return Ok(Some((owner, m.local)));
        }
        cur = vm.heap.get(owner)?.parent;
    }
    Ok(None)
}

fn resolve_cached_named_type(
    vm: &Vm,
    path: &[PklString],
    name_spans: &[Span],
    span: Span,
    ctx: &TypeCtx,
) -> Result<Option<Value>, RuntimeError> {
    let Some(scope_mod) = resolve_scope_module_if_loaded(vm, ctx)? else {
        return Ok(None);
    };
    if path.len() == 1 {
        let name = &path[0];
        let Some((owner, local)) =
            locate_simple_type_owner(vm, scope_mod, &ctx.source.uri, name, span)?
        else {
            return Ok(None);
        };
        cached_member_value(vm, owner, name, local)
    } else if path.len() == 2 && name_spans.len() == 2 {
        let import_member = vm.declared_type_import(scope_mod, &path[0], name_spans[0])?;
        let Some(Value::Object(imported_mod)) =
            cached_member_value(vm, scope_mod, &path[0], import_member.local)?
        else {
            return Ok(None);
        };
        let Some((owner, local)) = locate_qualified_type_owner(
            vm,
            imported_mod,
            &path[1],
            Span::new(name_spans[0].start, name_spans[1].end),
        )?
        else {
            return Ok(None);
        };
        cached_member_value(vm, owner, &path[1], local)
    } else {
        Ok(None)
    }
}

struct SchemaBuilder<'a> {
    vm: &'a Vm,
    decoded_annotations: &'a BTreeMap<ObjectId, DecodedObject>,
    store: SchemaStore,
    class_ids: BTreeMap<ObjectId, ClassId>,
    alias_ids: BTreeMap<ObjectId, TypeAliasId>,
    source_indices: BTreeMap<usize, crate::source::SourceTextIndex>,
}

impl<'a> SchemaBuilder<'a> {
    fn export_module(mut self, module_id: ObjectId) -> Result<ModuleSchema, RuntimeError> {
        let mut raw = self.build_module_raw(module_id)?;
        let store = Arc::new(self.store);
        attach_store(&mut raw, &store);
        Ok(raw)
    }

    fn build_module_raw(&mut self, module_id: ObjectId) -> Result<ModuleSchema, RuntimeError> {
        let obj = self.vm.heap.get(module_id)?;
        let unit = obj
            .module_unit
            .and_then(|u| self.vm.program.units.get(u.0 as usize))
            .ok_or_else(|| invalid("module schema unit missing"))?
            .clone();
        let source = unit
            .source
            .clone()
            .ok_or_else(|| invalid("module schema source missing"))?;
        let syntax = unit
            .syntax
            .clone()
            .ok_or_else(|| invalid("module schema syntax missing"))?;

        let supermodule = if let Some(parent) = obj
            .parent
            .filter(|p| self.vm.heap.get(*p).is_ok_and(|o| o.is_module))
        {
            Some(Box::new(self.build_module_raw(parent)?))
        } else {
            None
        };

        let module_uri = unit.identity.as_ref().map_or_else(
            || source.uri.to_java_utf8(),
            |id| id.original_uri.to_java_utf8(),
        );
        let module_name = unit.name.to_java_utf8();
        let is_amend = unit.mode == modules::ModuleMode::Amends;
        let doc_comment = syntax
            .declaration
            .as_ref()
            .and_then(|d| d.doc_comment.as_ref())
            .and_then(|doc| doc.text(&source.text))
            .map(|s| s.to_java_utf8());
        let annotations = self.decode_values(&obj.annotations);

        let mod_cls_obj = self
            .vm
            .heap
            .object_class(module_id)?
            .ok_or_else(|| invalid("module class missing"))?;
        let module_class_id = self.export_class(mod_cls_obj)?;

        // Imports in exact declaration order (`ModuleInfo.java:130-138` / `declarations.rs:942-970`)
        let mut imports = Vec::new();
        for member in &syntax.members {
            if let crate::syntax::ExprKind::Import {
                clause_alias: Some(_),
                ..
            } = &member.expr.kind
                && let Some(compiled) = obj
                    .members
                    .iter()
                    .find(|c| c.name == member.name && c.import())
                && let Some(block) = self.vm.program.blocks.get(compiled.code)
                && let Some(uri) = block.iter().find_map(|inst| match inst {
                    Instruction::ImportModule(spec) => Some(spec.absolute_uri.clone()),
                    Instruction::ImportGlob(spec) => {
                        spec.import.as_ref().map(|i| i.absolute_uri.clone())
                    }
                    _ => None,
                })
            {
                imports.push((member.name.to_java_utf8(), uri.to_java_utf8()));
            }
        }

        let mut classes = Vec::new();
        let mut type_aliases = Vec::new();
        if !is_amend {
            for member in &syntax.members {
                if member.local {
                    continue;
                }
                match member.expr.kind {
                    crate::syntax::ExprKind::Class { .. } => {
                        if let Some(Value::Class(cid)) =
                            cached_member_value(self.vm, module_id, &member.name, false)?
                        {
                            let exported = self.export_class(cid)?;
                            classes.push((member.name.to_java_utf8(), exported));
                        }
                    }
                    crate::syntax::ExprKind::TypeAlias { .. } => {
                        if let Some(Value::TypeAlias(tid)) =
                            cached_member_value(self.vm, module_id, &member.name, false)?
                        {
                            let exported = self.export_type_alias(tid)?;
                            type_aliases.push((member.name.to_java_utf8(), exported));
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(ModuleSchema {
            module_uri,
            module_name,
            is_amend,
            supermodule,
            module_class_id,
            doc_comment,
            annotations,
            classes,
            type_aliases,
            imports,
            store: Arc::new(SchemaStore::default()),
        })
    }

    fn export_class(&mut self, class_obj_id: ObjectId) -> Result<ClassId, RuntimeError> {
        if let Some(&id) = self.class_ids.get(&class_obj_id) {
            return Ok(id);
        }
        let rec = self.vm.heap.get_class(class_obj_id)?;
        let prototype = rec.prototype;
        let proto_obj = self.vm.heap.get(prototype)?;
        let is_module_class = proto_obj.is_module;

        // Export enclosing module class first (breaks cycles identically to VmClass.java:633-641)
        let enclosing_module_class = if !is_module_class {
            let origin = class_origin_uri(self.vm, class_obj_id)?;
            if let Some(enc_mod) = find_enclosing_module_if_loaded(self.vm, prototype, &origin) {
                let enc_cls = self
                    .vm
                    .heap
                    .object_class(enc_mod)?
                    .ok_or_else(|| invalid("enclosing module class missing"))?;
                Some(self.export_class(enc_cls)?)
            } else {
                None
            }
        } else {
            None
        };
        if let Some(&id) = self.class_ids.get(&class_obj_id) {
            return Ok(id);
        }

        let (info, doc_comment, source_location, modifiers, annotations, type_parameters) =
            if is_module_class {
                let unit = proto_obj
                    .module_unit
                    .and_then(|u| self.vm.program.units.get(u.0 as usize))
                    .ok_or_else(|| invalid("module class unit missing"))?
                    .clone();
                let source = unit
                    .source
                    .clone()
                    .ok_or_else(|| invalid("module class source missing"))?;
                let syntax = unit
                    .syntax
                    .clone()
                    .ok_or_else(|| invalid("module class syntax missing"))?;
                let module_uri = unit.identity.as_ref().map_or_else(
                    || source.uri.to_java_utf8(),
                    |id| id.original_uri.to_java_utf8(),
                );
                let info = PClassInfo {
                    module_name: unit.name.to_java_utf8(),
                    simple_name: PClassInfo::MODULE_CLASS_NAME.to_string(),
                    module_uri,
                    is_module_class: true,
                };
                let doc_comment = syntax
                    .declaration
                    .as_ref()
                    .and_then(|d| d.doc_comment.as_ref())
                    .and_then(|doc| doc.text(&source.text))
                    .map(|s| s.to_java_utf8());
                let header_span = syntax
                    .declaration
                    .as_ref()
                    .map_or(Span::default(), |d| d.span);
                let full_span = Span::new(header_span.start, source.text.len_bytes());
                let source_location = self.source_location(&source, header_span, full_span);
                let flags = if unit.mode == modules::ModuleMode::Amends {
                    0
                } else {
                    unit.modifiers
                };
                let modifiers = export_modifiers(flags, true);
                let annotations = self.decode_values(&proto_obj.annotations);
                (
                    info,
                    doc_comment,
                    source_location,
                    modifiers,
                    annotations,
                    Vec::new(),
                )
            } else {
                let def = self.vm.heap.class_definition(class_obj_id)?;
                let decl = schema_class_declaration(self.vm, class_obj_id)?;
                let module_name = def.module.as_ref().map_or_else(
                    || decl.source.module_name.to_java_utf8(),
                    |m| m.to_java_utf8(),
                );
                let module_uri = def
                    .origin_uri
                    .as_ref()
                    .map_or_else(|| decl.source.uri.to_java_utf8(), |u| u.to_java_utf8());
                let info = PClassInfo {
                    module_name,
                    simple_name: def.name.to_java_utf8(),
                    module_uri,
                    is_module_class: false,
                };
                let doc_comment = decl
                    .syntax
                    .doc_comment
                    .as_ref()
                    .and_then(|d| d.text(&decl.source.text))
                    .map(|s| s.to_java_utf8());
                let source_location = self.source_location(
                    &decl.source,
                    decl.syntax.header_span,
                    decl.syntax.declaration_span,
                );
                let flags = crate::syntax::modifier_flags(&decl.syntax.modifiers);
                let modifiers = export_modifiers(flags, true);
                let raw_anns = existing_decl_annotations(self.vm, &decl)?;
                let annotations = self.decode_values(&raw_anns);
                let type_parameters = decl
                    .parameters()
                    .iter()
                    .map(export_type_parameter)
                    .collect();
                (
                    info,
                    doc_comment,
                    source_location,
                    modifiers,
                    annotations,
                    type_parameters,
                )
            };

        // Reserve ClassId BEFORE recursing into superclass/properties/methods to break cycles.
        let id = ClassId(self.store.classes.len());
        self.class_ids.insert(class_obj_id, id);
        self.store.classes.push(PClass {
            id,
            info: info.clone(),
            doc_comment,
            source_location,
            modifiers,
            annotations,
            type_parameters,
            superclass: None,
            supertype: None,
            properties: Vec::new(),
            methods: Vec::new(),
            module_class: enclosing_module_class,
        });

        // Export superclass & supertype
        let superclass = if let Some(sup_obj) = rec.superclass {
            let sup_id = self.export_class(sup_obj)?;
            let supertype = if !is_module_class
                && let Ok(decl) = schema_class_declaration(self.vm, class_obj_id)
                && let DeclarationRecipe::Class {
                    superclass: Some(ref ty),
                    ..
                } = decl.syntax.recipe
            {
                let ctx = schema_type_context(self.vm, &decl)?;
                self.export_type(ty, &ctx)?
            } else {
                let sup_info = self.store.class(sup_id).info.clone();
                PType::Class {
                    class_id: sup_id,
                    class_info: sup_info,
                    type_arguments: Vec::new(),
                }
            };
            self.store.classes[id.0].superclass = Some(sup_id);
            self.store.classes[id.0].supertype = Some(supertype);
            Some(sup_id)
        } else {
            None
        };
        let _ = superclass;

        // Export own properties
        let prop_decls = schema_class_members(self.vm, class_obj_id, false)?;
        let mut properties = Vec::with_capacity(prop_decls.len());
        for decl in prop_decls {
            let ctx = schema_type_context(self.vm, &decl)?;
            let ty = match &decl.syntax.recipe {
                DeclarationRecipe::Property { ty: Some(t), .. } => self.export_type(t, &ctx)?,
                _ => PType::Unknown,
            };
            let doc_comment = decl
                .syntax
                .doc_comment
                .as_ref()
                .and_then(|d| d.text(&decl.source.text))
                .map(|s| s.to_java_utf8());
            let source_location = self.source_location(
                &decl.source,
                decl.syntax.header_span,
                decl.syntax.declaration_span,
            );
            let flags = crate::syntax::modifier_flags(&decl.syntax.modifiers);
            let modifiers = export_modifiers(flags, false);
            let raw_anns = existing_decl_annotations(self.vm, &decl)?;
            let annotations = self.decode_values(&raw_anns);
            let name = decl.syntax.name.to_java_utf8();
            properties.push((
                name.clone(),
                PClassProperty {
                    owner: id,
                    owner_info: info.clone(),
                    simple_name: name,
                    doc_comment,
                    source_location,
                    modifiers,
                    annotations,
                    ty,
                },
            ));
        }
        self.store.classes[id.0].properties = properties;

        // Export own methods
        let method_decls = schema_class_members(self.vm, class_obj_id, true)?;
        let mut methods = Vec::with_capacity(method_decls.len());
        for decl in method_decls {
            let ctx = schema_type_context(self.vm, &decl)?;
            let (type_parameters, parameters, return_type) = match &decl.syntax.recipe {
                DeclarationRecipe::Method {
                    type_parameters,
                    parameters,
                    return_type,
                } => {
                    let tps = type_parameters.iter().map(export_type_parameter).collect();
                    let mut params = Vec::with_capacity(parameters.len());
                    for (idx, param) in parameters.iter().enumerate() {
                        let pname = if param.name == "_" {
                            format!("_#{idx}")
                        } else {
                            param.name.to_java_utf8()
                        };
                        let pty = match &param.ty {
                            Some(t) => self.export_type(t, &ctx)?,
                            None => PType::Unknown,
                        };
                        params.push((pname, pty));
                    }
                    let ret = match return_type {
                        Some(t) => self.export_type(t, &ctx)?,
                        None => PType::Unknown,
                    };
                    (tps, params, ret)
                }
                _ => (Vec::new(), Vec::new(), PType::Unknown),
            };
            let doc_comment = decl
                .syntax
                .doc_comment
                .as_ref()
                .and_then(|d| d.text(&decl.source.text))
                .map(|s| s.to_java_utf8());
            let source_location = self.source_location(
                &decl.source,
                decl.syntax.header_span,
                decl.syntax.declaration_span,
            );
            let flags = crate::syntax::modifier_flags(&decl.syntax.modifiers);
            let modifiers = export_modifiers(flags, false);
            let raw_anns = existing_decl_annotations(self.vm, &decl)?;
            let annotations = self.decode_values(&raw_anns);
            let name = decl.syntax.name.to_java_utf8();
            methods.push((
                name.clone(),
                PClassMethod {
                    owner: id,
                    owner_info: info.clone(),
                    simple_name: name,
                    doc_comment,
                    source_location,
                    modifiers,
                    annotations,
                    type_parameters,
                    parameters,
                    return_type,
                },
            ));
        }
        self.store.classes[id.0].methods = methods;

        Ok(id)
    }

    fn export_type_alias(&mut self, alias_obj_id: ObjectId) -> Result<TypeAliasId, RuntimeError> {
        if let Some(&id) = self.alias_ids.get(&alias_obj_id) {
            return Ok(id);
        }
        let decl = schema_alias_declaration(self.vm, alias_obj_id)?;
        let obj = self.vm.heap.get(alias_obj_id)?;
        let simple_name = decl.syntax.name.to_java_utf8();
        let (module_name, qualified_name) = if let Some(q) = obj.type_alias.as_ref() {
            let qs = q.to_java_utf8();
            let mn = qs.rsplit_once('#').map_or_else(
                || decl.source.module_name.to_java_utf8(),
                |(m, _)| m.to_string(),
            );
            (mn, qs)
        } else {
            let mn = decl.source.module_name.to_java_utf8();
            let qs = format!("{mn}#{simple_name}");
            (mn, qs)
        };
        let enc_mod =
            find_enclosing_module_if_loaded(self.vm, decl.owner, &Some(decl.source.uri.clone()))
                .ok_or_else(|| invalid("type alias enclosing module missing"))?;
        let enc_cls_obj = self
            .vm
            .heap
            .object_class(enc_mod)?
            .ok_or_else(|| invalid("type alias enclosing module class missing"))?;
        let module_class = self.export_class(enc_cls_obj)?;

        let doc_comment = decl
            .syntax
            .doc_comment
            .as_ref()
            .and_then(|d| d.text(&decl.source.text))
            .map(|s| s.to_java_utf8());
        let source_location = self.source_location(
            &decl.source,
            decl.syntax.header_span,
            decl.syntax.declaration_span,
        );
        let flags = crate::syntax::modifier_flags(&decl.syntax.modifiers);
        let modifiers = export_modifiers(flags, true);
        let raw_anns = existing_decl_annotations(self.vm, &decl)?;
        let annotations = self.decode_values(&raw_anns);
        let type_parameters = decl
            .parameters()
            .iter()
            .map(export_type_parameter)
            .collect();

        // Reserve TypeAliasId before exporting referent to break cycles through classes.
        let id = TypeAliasId(self.store.type_aliases.len());
        self.alias_ids.insert(alias_obj_id, id);
        self.store.type_aliases.push(TypeAlias {
            id,
            info: TypeAliasInfo {
                module_name,
                simple_name,
                qualified_name,
            },
            doc_comment,
            source_location,
            modifiers,
            annotations,
            type_parameters,
            aliased_type: PType::Unknown,
            module_class,
        });

        if let DeclarationRecipe::TypeAlias { ref referent, .. } = decl.syntax.recipe {
            let ctx = schema_type_context(self.vm, &decl)?;
            let aliased_type = self.export_type(referent, &ctx)?;
            self.store.type_aliases[id.0].aliased_type = aliased_type;
        }

        Ok(id)
    }

    fn export_type(&mut self, ty: &Type, ctx: &TypeCtx) -> Result<PType, RuntimeError> {
        match &ty.kind {
            TypeKind::Unknown => Ok(PType::Unknown),
            TypeKind::Nothing => Ok(PType::Nothing),
            TypeKind::Module => Ok(PType::Module),
            TypeKind::String(lit) => Ok(PType::StringLiteral(lit.to_java_utf8())),
            TypeKind::Nullable(inner) => {
                Ok(PType::Nullable(Box::new(self.export_type(inner, ctx)?)))
            }
            TypeKind::Union { types, .. } => {
                let mut elems = Vec::with_capacity(types.len());
                for t in types {
                    elems.push(self.export_type(t, ctx)?);
                }
                Ok(PType::Union(elems))
            }
            TypeKind::Function {
                parameters,
                return_type,
            } => {
                let mut params = Vec::with_capacity(parameters.len());
                for t in parameters {
                    params.push(self.export_type(t, ctx)?);
                }
                let ret = self.export_type(return_type, ctx)?;
                Ok(PType::Function {
                    parameter_types: params,
                    return_type: Box::new(ret),
                })
            }
            TypeKind::Constrained { base, constraints } => {
                let base_type = self.export_type(base, ctx)?;
                let mut texts = Vec::with_capacity(constraints.len());
                for expr in constraints {
                    let slice = ctx
                        .source
                        .text
                        .slice(expr.span)
                        .map(|s| s.to_pkl_string().to_java_utf8())
                        .unwrap_or_default();
                    texts.push(slice);
                }
                Ok(PType::Constrained {
                    base_type: Box::new(base_type),
                    constraints: texts,
                })
            }
            TypeKind::Named {
                path,
                name_spans,
                arguments,
            } => {
                if path.len() == 1
                    && let Some(tp) = ctx.parameters.iter().find(|p| p.name == path[0])
                {
                    return Ok(PType::TypeVariable(export_type_parameter(tp)));
                }
                let val = resolve_cached_named_type(self.vm, path, name_spans, ty.span, ctx)?
                    .ok_or_else(|| {
                        invalid(&format!(
                            "unresolved named type `{}` during schema export",
                            path.iter()
                                .map(|p| p.to_java_utf8())
                                .collect::<Vec<_>>()
                                .join(".")
                        ))
                    })?;
                match val {
                    Value::Class(class_obj_id) => {
                        let is_erased = self.vm.canonical_type_class(class_obj_id)?
                            == Some(BuiltinClass::Class);
                        let class_id = self.export_class(class_obj_id)?;
                        let class_info = self.store.class(class_id).info.clone();
                        let type_arguments = if is_erased {
                            Vec::new()
                        } else {
                            let mut args = Vec::with_capacity(arguments.len());
                            for a in arguments {
                                args.push(self.export_type(a, ctx)?);
                            }
                            args
                        };
                        Ok(PType::Class {
                            class_id,
                            class_info,
                            type_arguments,
                        })
                    }
                    Value::Object(mod_id) => {
                        let mc = self.vm.declared_module_class(mod_id, ty.span)?;
                        let class_id = self.export_class(mc)?;
                        let class_info = self.store.class(class_id).info.clone();
                        Ok(PType::Class {
                            class_id,
                            class_info,
                            type_arguments: Vec::new(),
                        })
                    }
                    Value::TypeAlias(alias_obj_id) => {
                        let alias_id = self.export_type_alias(alias_obj_id)?;
                        let mut type_arguments = Vec::with_capacity(arguments.len());
                        for a in arguments {
                            type_arguments.push(self.export_type(a, ctx)?);
                        }
                        let alias = self.store.type_alias(alias_id).clone();
                        let aliased_type = substitute_type_params(
                            &alias.aliased_type,
                            &alias.type_parameters,
                            &type_arguments,
                        );
                        Ok(PType::Alias {
                            alias_id,
                            alias_info: alias.info,
                            type_arguments,
                            aliased_type: Box::new(aliased_type),
                        })
                    }
                    _ => Err(invalid("unexpected value resolved for named type")),
                }
            }
        }
    }

    fn decode_values(&self, values: &[Value]) -> Vec<DecodedObject> {
        values
            .iter()
            .filter_map(|v| v.heap_id())
            .filter_map(|id| self.decoded_annotations.get(&id).cloned())
            .collect()
    }

    fn source_location(
        &mut self,
        source: &Arc<RuntimeSource>,
        header_span: Span,
        decl_span: Span,
    ) -> SourceLocation {
        let key = Arc::as_ptr(source) as usize;
        let idx = self
            .source_indices
            .entry(key)
            .or_insert_with(|| source.text.index());
        let start_byte = clamp_char_boundary(&source.text, header_span.start);
        let start_line = idx.position(start_byte).map(|p| p.line as u32).unwrap_or(1);
        let raw_end = if decl_span.end > decl_span.start {
            decl_span.end - 1
        } else {
            decl_span.start
        };
        let end_byte = clamp_char_boundary(&source.text, raw_end);
        let end_line = idx
            .position(end_byte)
            .map(|p| p.line as u32)
            .unwrap_or(start_line);
        SourceLocation {
            start_line,
            end_line,
        }
    }
}

fn clamp_char_boundary(text: &crate::source::SourceText, byte: usize) -> usize {
    let bytes = text.as_bytes();
    if bytes.is_empty() {
        return 0;
    }
    let mut b = byte.min(bytes.len().saturating_sub(1));
    while b > 0 && (bytes[b] & 0b1100_0000) == 0b1000_0000 {
        b -= 1;
    }
    b
}

fn attach_store(schema: &mut ModuleSchema, store: &Arc<SchemaStore>) {
    schema.store = Arc::clone(store);
    if let Some(sup) = schema.supermodule.as_deref_mut() {
        attach_store(sup, store);
    }
}

fn export_modifiers(flags: u32, is_class: bool) -> Vec<Modifier> {
    use crate::syntax::ModifierKind as M;
    let mut out = Vec::new();
    if flags & M::Abstract.flag() != 0 {
        out.push(Modifier::Abstract);
    }
    if flags & M::Open.flag() != 0 {
        out.push(Modifier::Open);
    }
    if flags & M::Hidden.flag() != 0 {
        out.push(Modifier::Hidden);
    }
    if is_class && (flags & M::External.flag() != 0) {
        out.push(Modifier::External);
    }
    out
}

fn export_type_parameter(tp: &crate::syntax::TypeParameter) -> TypeParameter {
    TypeParameter {
        name: tp.name.to_java_utf8(),
        variance: match tp.variance {
            crate::syntax::Variance::Invariant => Variance::Invariant,
            crate::syntax::Variance::Out => Variance::Covariant,
            crate::syntax::Variance::In => Variance::Contravariant,
        },
    }
}

fn substitute_type_params(ty: &PType, params: &[TypeParameter], args: &[PType]) -> PType {
    if params.is_empty() {
        return ty.clone();
    }
    match ty {
        PType::TypeVariable(tp) => {
            if let Some(idx) = params.iter().position(|p| p.name == tp.name) {
                args.get(idx).cloned().unwrap_or(PType::Unknown)
            } else {
                ty.clone()
            }
        }
        PType::Class {
            class_id,
            class_info,
            type_arguments,
        } => PType::Class {
            class_id: *class_id,
            class_info: class_info.clone(),
            type_arguments: type_arguments
                .iter()
                .map(|a| substitute_type_params(a, params, args))
                .collect(),
        },
        PType::Nullable(inner) => {
            PType::Nullable(Box::new(substitute_type_params(inner, params, args)))
        }
        PType::Union(elems) => PType::Union(
            elems
                .iter()
                .map(|e| substitute_type_params(e, params, args))
                .collect(),
        ),
        PType::Function {
            parameter_types,
            return_type,
        } => PType::Function {
            parameter_types: parameter_types
                .iter()
                .map(|p| substitute_type_params(p, params, args))
                .collect(),
            return_type: Box::new(substitute_type_params(return_type, params, args)),
        },
        PType::Constrained {
            base_type,
            constraints,
        } => PType::Constrained {
            base_type: Box::new(substitute_type_params(base_type, params, args)),
            constraints: constraints.clone(),
        },
        PType::Alias {
            alias_id,
            alias_info,
            type_arguments,
            aliased_type,
        } => PType::Alias {
            alias_id: *alias_id,
            alias_info: alias_info.clone(),
            type_arguments: type_arguments
                .iter()
                .map(|a| substitute_type_params(a, params, args))
                .collect(),
            aliased_type: Box::new(substitute_type_params(aliased_type, params, args)),
        },
        PType::Unknown | PType::Nothing | PType::Module | PType::StringLiteral(_) => ty.clone(),
    }
}
