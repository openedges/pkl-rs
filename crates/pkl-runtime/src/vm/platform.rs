//! Platform host module data, typed instantiation, and cooperative Benchmark runners.
use super::*;
use crate::heap::CollectionKind;
use crate::value::RuntimeSource;

const KIND_MICRO: i64 = 0;
const KIND_OUTPUT: i64 = 1;
const KIND_PARSER: i64 = 2;

const PHASE_READ_SOURCE_MODULE: i64 = 1;
const PHASE_READ_SOURCE_TEXT: i64 = 2;
const PHASE_READ_SOURCE_URI: i64 = 3;
const PHASE_READ_ITERATIONS: i64 = 4;
const PHASE_READ_ITERATION_TIME: i64 = 5;
const PHASE_READ_IS_VERBOSE: i64 = 6;
const PHASE_WARMUP_STEP: i64 = 7;
const PHASE_MEASURE_STEP: i64 = 8;
const PHASE_FINISH: i64 = 9;

const OUT_STAGE_IDLE: i64 = 0;
const OUT_STAGE_RELOADED: i64 = 1;
const OUT_STAGE_OUTPUT: i64 = 2;
const OUT_STAGE_TEXT: i64 = 3;

pub(super) struct OutputBenchmarkRestore {
    pub(super) unit_id: crate::modules::UnitId,
    pub(super) std_key: PklString,
    pub(super) original_uri: PklString,
    pub(super) resolved_uri: PklString,
    pub(super) saved_std: Option<modules::CachedModule>,
    pub(super) saved_orig: Option<modules::CachedModule>,
    pub(super) saved_res: Option<modules::CachedModule>,
    pub(super) saved_module_index: Option<ObjectId>,
    pub(super) saved_class_index: crate::heap::FastMap<usize, ObjectId>,
}

const MAX_BENCHMARK_REPETITIONS: i64 = 1_000_000;

fn bench_key(name: &str) -> CacheKey {
    CacheKey {
        owner: None,
        name: name.into(),
        method: false,
        entry: false,
    }
}

fn duration_to_nanos(d: crate::duration::Duration) -> i64 {
    let factor = match d.unit() {
        crate::duration::DurationUnit::Nanoseconds => 1.0,
        crate::duration::DurationUnit::Microseconds => 1_000.0,
        crate::duration::DurationUnit::Milliseconds => 1_000_000.0,
        crate::duration::DurationUnit::Seconds => 1_000_000_000.0,
        crate::duration::DurationUnit::Minutes => 60_000_000_000.0,
        crate::duration::DurationUnit::Hours => 3_600_000_000_000.0,
        crate::duration::DurationUnit::Days => 86_400_000_000_000.0,
    };
    let nanos = (d.value() * factor).round();
    if !nanos.is_finite() || nanos <= 0.0 {
        0
    } else if nanos >= i64::MAX as f64 {
        i64::MAX
    } else {
        nanos as i64
    }
}

fn choose_benchmark_unit(mean_nanos: f64) -> (crate::duration::DurationUnit, f64) {
    if mean_nanos < 1_000.0 {
        (crate::duration::DurationUnit::Nanoseconds, 1.0)
    } else if mean_nanos < 1_000_000.0 {
        (crate::duration::DurationUnit::Microseconds, 1_000.0)
    } else if mean_nanos < 1_000_000_000.0 {
        (crate::duration::DurationUnit::Milliseconds, 1_000_000.0)
    } else {
        (crate::duration::DurationUnit::Seconds, 1_000_000_000.0)
    }
}

fn advance_warmup_repetitions(state: i64, repetitions: i64) -> (i64, i64) {
    let (next_state, next_reps) = if state == 1 {
        (2, repetitions.saturating_mul(2))
    } else if state == 2 {
        (5, repetitions.saturating_mul(5) / 2)
    } else {
        (1, repetitions.saturating_mul(2))
    };
    (next_state, next_reps.max(1))
}

fn is_constant_expression_block(block: &[Instruction]) -> bool {
    let mut has_value = false;
    for instruction in block {
        match instruction {
            Instruction::Constant(_)
            | Instruction::MakeList { .. }
            | Instruction::BeginCollection {
                kind: CollectionKind::Set | CollectionKind::Map,
                has_parent: false,
                ..
            }
            | Instruction::PrepareSetElement
            | Instruction::AddSetElement
            | Instruction::PrepareMapEntry
            | Instruction::AddMapEntry
            | Instruction::EndCollection => {
                has_value = true;
            }
            Instruction::Observe(_) | Instruction::Return => {}
            _ => return false,
        }
    }
    has_value
}

impl Vm {
    pub(super) fn get_or_create_module_class(
        &mut self,
        module_id: ObjectId,
        class_name: &str,
    ) -> Result<ObjectId, RuntimeError> {
        let object = self.heap.get(module_id)?;
        let member = object
            .members
            .iter()
            .find(|m| m.name == class_name && !m.entry && !m.method)
            .ok_or_else(|| invalid("missing class member"))?
            .clone();
        let block = self
            .program
            .blocks
            .get(member.code)
            .ok_or_else(|| invalid("missing class block"))?;
        let Some(Instruction::BeginClass {
            template, class, ..
        }) = block.first().cloned()
        else {
            return Err(invalid("member does not define a class"));
        };
        if let Some(id) = self.class_singletons.get(&template).copied() {
            return Ok(id);
        }
        let origin_uri = class.origin_uri.clone();
        let environment = self
            .frames
            .last()
            .expect("active frame")
            .environment
            .clone();
        let mut class = class;
        class.origin_uri = origin_uri;
        class.module = class
            .module
            .or_else(|| Some(self.current_module_name().into()));
        let id = self.heap.allocate(Object {
            class_definition: Some(class),
            class_initializing: true,
            declaration_code: Some(member.code),
            environment,
            ..Object::default()
        });
        let class_id = self.heap.allocate_class(id);
        let parent = self.canonical_builtin_class(classes::BuiltinClass::Typed);
        self.heap.link_superclass(class_id, Some(parent))?;
        if let Some(members) = self.program.objects.get(template) {
            self.heap.get_mut(id)?.members = members.clone().into();
        }
        self.heap.get_mut(id)?.class_initializing = false;
        self.class_singletons.insert(template, class_id);
        self.declaration_roots_dirty = true;
        Ok(class_id)
    }

    pub(super) fn instantiate_typed(
        &mut self,
        class_id: ObjectId,
        properties: Vec<(&str, Value)>,
    ) -> Result<ObjectId, RuntimeError> {
        let prototype = self.heap.get_class(class_id)?.prototype;
        let mut object = Object {
            parent: Some(prototype),
            instance_class: Some(class_id),
            ..Object::default()
        };
        for (name, value) in properties {
            let name: PklString = name.into();
            object.members.push(Member {
                flags: 0,
                name: name.clone(),
                code: usize::MAX,
                local: false,
                method: false,
                entry: false,
                ty: None,
                type_span: None,
                constant: false,
                fixed: false,
            });
            object.member_values.insert(
                CacheKey {
                    owner: None,
                    name,
                    method: false,
                    entry: false,
                },
                value,
            );
        }
        Ok(self.heap.allocate(object))
    }

    #[cfg_attr(target_arch = "wasm32", inline(never))]
    pub(super) fn platform_current(&mut self) -> Result<(), RuntimeError> {
        let Some(platform_meta) = self.platform_metadata.clone() else {
            return Err(RuntimeError::new(
                ErrorKind::Unsupported,
                "This standard-library current value requires explicit platform/release Host data.",
            ));
        };
        let module_id = self
            .frames
            .last()
            .and_then(|f| f.environment.this_value.as_ref())
            .and_then(|v| v.heap_id())
            .or_else(|| {
                if let Some(modules::CachedModule::Value(id)) =
                    self.modules_stdlib.get("pkl:platform")
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .ok_or_else(|| invalid("platform module receiver"))?;

        let lang_class = self.get_or_create_module_class(module_id, "Language")?;
        let lang =
            self.instantiate_typed(lang_class, vec![("version", Value::string("0.32.0"))])?;

        let rt_class = self.get_or_create_module_class(module_id, "Runtime")?;
        let rt = self.instantiate_typed(
            rt_class,
            vec![
                ("name", Value::string("pkl-rust")),
                ("version", Value::string("0.32.0")),
            ],
        )?;

        let vm_class = self.get_or_create_module_class(module_id, "VirtualMachine")?;
        let vm = self.instantiate_typed(
            vm_class,
            vec![
                ("name", Value::string("pkl-runtime")),
                ("version", Value::string("0.32.0")),
            ],
        )?;

        let os_class = self.get_or_create_module_class(module_id, "OperatingSystem")?;
        let os = self.instantiate_typed(
            os_class,
            vec![
                ("name", Value::String(platform_meta.os_name)),
                ("version", Value::String(platform_meta.os_version)),
            ],
        )?;

        let proc_class = self.get_or_create_module_class(module_id, "Processor")?;
        let proc = self.instantiate_typed(
            proc_class,
            vec![(
                "architecture",
                Value::String(platform_meta.cpu_architecture),
            )],
        )?;

        let plat_class = self.get_or_create_module_class(module_id, "Platform")?;
        let plat = self.instantiate_typed(
            plat_class,
            vec![
                ("language", Value::Object(lang)),
                ("runtime", Value::Object(rt)),
                ("virtualMachine", Value::Object(vm)),
                ("operatingSystem", Value::Object(os)),
                ("processor", Value::Object(proc)),
            ],
        )?;

        self.stack.push(Value::Object(plat));
        Ok(())
    }

    fn benchmark_set(
        &mut self,
        state_id: ObjectId,
        key: &str,
        value: Value,
    ) -> Result<(), RuntimeError> {
        self.heap
            .get_mut(state_id)?
            .member_values
            .insert(bench_key(key), value);
        Ok(())
    }

    fn benchmark_get(&self, state_id: ObjectId, key: &str) -> Result<Option<Value>, RuntimeError> {
        Ok(self
            .heap
            .get(state_id)?
            .member_values
            .get(&bench_key(key))
            .cloned())
    }

    fn benchmark_get_i64(&self, state_id: ObjectId, key: &str) -> Result<i64, RuntimeError> {
        match self.benchmark_get(state_id, key)? {
            Some(Value::Int(n)) => Ok(n.get()),
            _ => Err(invalid("missing benchmark integer state")),
        }
    }

    fn benchmark_set_u128(
        &mut self,
        state_id: ObjectId,
        prefix: &str,
        value: u128,
    ) -> Result<(), RuntimeError> {
        let lo = (value & u64::MAX as u128) as u64 as i64;
        let hi = (value >> 64) as u64 as i64;
        self.benchmark_set(state_id, &format!("{prefix}_lo"), Value::Int(lo.into()))?;
        self.benchmark_set(state_id, &format!("{prefix}_hi"), Value::Int(hi.into()))
    }

    fn benchmark_get_u128(&self, state_id: ObjectId, prefix: &str) -> Result<u128, RuntimeError> {
        let lo = self.benchmark_get_i64(state_id, &format!("{prefix}_lo"))? as u64 as u128;
        let hi = self.benchmark_get_i64(state_id, &format!("{prefix}_hi"))? as u64 as u128;
        Ok((hi << 64) | lo)
    }

    fn benchmark_now_ns(&mut self, _state_id: ObjectId) -> Result<u128, RuntimeError> {
        if let Some(clock) = &self.monotonic_clock {
            return Ok(clock());
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            static EPOCH: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            let epoch = EPOCH.get_or_init(std::time::Instant::now);
            Ok(epoch.elapsed().as_nanos())
        }
        #[cfg(target_arch = "wasm32")]
        {
            let next = self
                .benchmark_get_i64(_state_id, "wasm_clock_ns")
                .unwrap_or(0)
                .saturating_add(1_000);
            self.benchmark_set(_state_id, "wasm_clock_ns", Value::Int(next.into()))?;
            Ok(next as u128)
        }
    }

    fn benchmark_kind(&self, this_id: ObjectId) -> Result<i64, RuntimeError> {
        let mut current = self.heap.object_class(this_id)?;
        while let Some(class_id) = current {
            let name = self.heap.class_simple_name(class_id)?;
            match name.as_str() {
                Some("Microbenchmark") => return Ok(KIND_MICRO),
                Some("OutputBenchmark") => return Ok(KIND_OUTPUT),
                Some("ParserBenchmark") => return Ok(KIND_PARSER),
                _ => {}
            }
            current = self.heap.get_class(class_id)?.superclass;
        }
        Err(invalid("benchmark receiver is not a Benchmark subclass"))
    }

    fn check_microbenchmark_non_constant(&self, this_id: ObjectId) -> Result<(), RuntimeError> {
        let mut current = Some(this_id);
        let mut visited = BTreeSet::new();
        while let Some(id) = current {
            if !visited.insert(id) {
                break;
            }
            let object = self.heap.get(id)?;
            if let Some(member) = object
                .members
                .iter()
                .find(|m| m.name == "expression" && !m.local && !m.method && !m.entry)
            {
                let is_undefined =
                    self.program.blocks.get(member.code).is_some_and(|block| {
                        matches!(block.first(), Some(Instruction::Undefined(_)))
                    });
                if !is_undefined {
                    let is_constant = self
                        .program
                        .metadata
                        .get(member.code)
                        .is_some_and(|meta| meta.constant_member)
                        || self
                            .program
                            .blocks
                            .get(member.code)
                            .is_some_and(|block| is_constant_expression_block(block));
                    if is_constant {
                        return Err(RuntimeError::new(
                            ErrorKind::Type,
                            "Microbenchmark is a constant expression.",
                        ));
                    }
                }
                return Ok(());
            }
            current = object.parent;
        }
        Ok(())
    }

    fn save_module_cache_slot(
        &mut self,
        state_id: ObjectId,
        key: &str,
        entry: Option<modules::CachedModule>,
    ) -> Result<(), RuntimeError> {
        match entry {
            Some(modules::CachedModule::Value(id)) => {
                self.benchmark_set(state_id, key, Value::Object(id))
            }
            _ => self.benchmark_set(state_id, key, Value::Null),
        }
    }

    fn restore_output_benchmark_caches(
        &mut self,
        state_id: ObjectId,
        src_mod_id: ObjectId,
    ) -> Result<(), RuntimeError> {
        if let Some(saved) = self
            .frames
            .last_mut()
            .and_then(|frame| frame.benchmark_restore.take())
        {
            self.apply_output_benchmark_restore(*saved);
            return Ok(());
        }
        let Some(unit_id) = self.heap.get(src_mod_id)?.module_unit else {
            return Ok(());
        };
        let Some(unit) = self.program.units.get(unit_id.0 as usize) else {
            return Ok(());
        };
        let Some(identity) = unit.identity.clone() else {
            return Ok(());
        };
        let std_uri = unit
            .singleton_cache_key()
            .map(PklString::from)
            .unwrap_or_else(|| identity.original_uri.clone());
        let restore = |cache: &mut modules::ModuleCache, uri: &PklString, saved: Option<Value>| {
            if let Some(Value::Object(id)) = saved {
                cache.insert(uri.clone(), modules::CachedModule::Value(id));
            } else {
                cache.remove(uri.clone());
            }
        };
        let saved_std = self.benchmark_get(state_id, "saved_std")?;
        let saved_orig = self.benchmark_get(state_id, "saved_orig")?;
        let saved_res = self.benchmark_get(state_id, "saved_res")?;
        self.last_warm_module = None;
        restore(&mut self.modules_stdlib, &std_uri, saved_std);
        restore(
            &mut self.modules_original,
            &identity.original_uri,
            saved_orig,
        );
        restore(
            &mut self.modules_resolved,
            &identity.resolved_uri,
            saved_res,
        );
        Ok(())
    }

    pub(super) fn apply_output_benchmark_restore(&mut self, saved: OutputBenchmarkRestore) {
        self.last_warm_module = None;
        let restore = |cache: &mut modules::ModuleCache,
                       uri: &PklString,
                       entry: Option<modules::CachedModule>| {
            if let Some(entry) = entry {
                cache.insert(uri.clone(), entry);
            } else {
                cache.remove(uri.clone());
            }
        };
        restore(&mut self.modules_stdlib, &saved.std_key, saved.saved_std);
        restore(
            &mut self.modules_original,
            &saved.original_uri,
            saved.saved_orig,
        );
        restore(
            &mut self.modules_resolved,
            &saved.resolved_uri,
            saved.saved_res,
        );
        self.heap
            .restore_local_module_index(saved.unit_id, saved.saved_module_index);
        self.heap.restore_local_class_index(saved.saved_class_index);
    }

    fn launch_benchmark_repetition(
        &mut self,
        this_id: ObjectId,
        state_id: ObjectId,
        kind: i64,
    ) -> Result<(), RuntimeError> {
        match kind {
            KIND_MICRO => {
                // Re-evaluate `expression` uncached with full lexical/module/outer scopes.
                self.frames.last_mut().unwrap().pc -= 1;
                self.force(this_id, this_id, "expression".into(), false, false, false)?;
                Ok(())
            }
            KIND_OUTPUT => {
                let Some(Value::Object(src_mod_id)) =
                    self.benchmark_get(state_id, "source_module")?
                else {
                    return Err(invalid("missing OutputBenchmark sourceModule"));
                };
                if let Some(unit_id) = self.heap.get(src_mod_id)?.module_unit {
                    let unit = self
                        .program
                        .units
                        .get(unit_id.0 as usize)
                        .ok_or_else(|| invalid("missing OutputBenchmark module unit"))?;
                    let entry = unit.entry;
                    let std_uri = unit.singleton_cache_key().map(PklString::from);
                    if let Some(identity) = unit.identity.clone() {
                        let std_key = std_uri.unwrap_or_else(|| identity.original_uri.clone());
                        let std_slot = self.modules_stdlib.get(&std_key).cloned();
                        let orig_slot = self.modules_original.get(&identity.original_uri).cloned();
                        let res_slot = self.modules_resolved.get(&identity.resolved_uri).cloned();
                        self.save_module_cache_slot(state_id, "saved_std", std_slot.clone())?;
                        self.save_module_cache_slot(state_id, "saved_orig", orig_slot.clone())?;
                        self.save_module_cache_slot(state_id, "saved_res", res_slot.clone())?;
                        let saved_module_index = self.heap.get_local_module_index(unit_id);
                        let saved_class_index = self.heap.snapshot_local_class_index();
                        self.frames.last_mut().unwrap().benchmark_restore =
                            Some(Box::new(OutputBenchmarkRestore {
                                unit_id,
                                std_key,
                                original_uri: identity.original_uri,
                                resolved_uri: identity.resolved_uri,
                                saved_std: std_slot,
                                saved_orig: orig_slot,
                                saved_res: res_slot,
                                saved_module_index,
                                saved_class_index,
                            }));
                    }
                    self.benchmark_set(
                        state_id,
                        "out_stage",
                        Value::Int(OUT_STAGE_RELOADED.into()),
                    )?;
                    self.frames.last_mut().unwrap().pc -= 1;
                    self.push_module_initializer(entry, None);
                    Ok(())
                } else {
                    // Fallback for amended module instances without a distinct UnitId.
                    self.benchmark_set(state_id, "out_stage", Value::Int(OUT_STAGE_OUTPUT.into()))?;
                    self.frames.last_mut().unwrap().pc -= 1;
                    self.force(src_mod_id, src_mod_id, "output".into(), false, false, false)?;
                    Ok(())
                }
            }
            KIND_PARSER => {
                let Some(Value::String(source_text)) =
                    self.benchmark_get(state_id, "source_text")?
                else {
                    return Err(invalid("missing ParserBenchmark sourceText"));
                };
                let Some(Value::String(source_uri)) = self.benchmark_get(state_id, "source_uri")?
                else {
                    return Err(invalid("missing ParserBenchmark sourceUri"));
                };
                let runtime_source = Arc::new(RuntimeSource {
                    text: crate::source::SourceText::from(&source_text),
                    uri: source_uri,
                    module_name: "pkl.Benchmark#ParserBenchmark.run".into(),
                });
                let parsed = crate::parser::parse_module_for_evaluation(&runtime_source.text)
                    .map_err(|failure| {
                        if let Some(err) =
                            failure.minimum_version_error(&runtime_source.module_name)
                        {
                            err
                        } else {
                            modules::imported_diagnostic(
                                failure.diagnostic,
                                runtime_source.clone(),
                                "pkl.Benchmark#ParserBenchmark.run".into(),
                            )
                        }
                    })?;
                let resolved = crate::resolver::resolve_module(&parsed).map_err(|diagnostic| {
                    modules::imported_diagnostic(
                        diagnostic,
                        runtime_source.clone(),
                        "pkl.Benchmark#ParserBenchmark.run".into(),
                    )
                })?;
                let _compiled = compile_module(&resolved)?;
                // Yield back to the VM scheduler after each parse repetition so
                // budgets, cancellation, and GC safepoints remain responsive.
                self.frames.last_mut().unwrap().pc -= 1;
                Ok(())
            }
            _ => Err(invalid("unknown benchmark kind")),
        }
    }

    fn finish_benchmark_repetition(
        &mut self,
        this_id: ObjectId,
        state_id: ObjectId,
        kind: i64,
        phase: i64,
    ) -> Result<(), RuntimeError> {
        let iterations = self.benchmark_get_i64(state_id, "iterations")?;
        let iter_ns = self.benchmark_get_i64(state_id, "iter_ns")?.max(0) as u128;

        if phase == PHASE_WARMUP_STEP {
            let warmup_reps = self.benchmark_get_i64(state_id, "warmup_reps")?;
            let rep_idx = self.benchmark_get_i64(state_id, "warmup_rep_idx")? + 1;
            if rep_idx < warmup_reps {
                self.benchmark_set(state_id, "warmup_rep_idx", Value::Int(rep_idx.into()))?;
                return self.launch_benchmark_repetition(this_id, state_id, kind);
            }
            // Completed one warmup batch of `warmup_reps` repetitions.
            let warmup_state = self.benchmark_get_i64(state_id, "warmup_state")?;
            let (next_state, next_reps) = advance_warmup_repetitions(warmup_state, warmup_reps);
            let warmup_start = self.benchmark_get_u128(state_id, "warmup_start")?;
            let elapsed = self
                .benchmark_now_ns(state_id)?
                .saturating_sub(warmup_start);
            let min_warmup_ns = ((iter_ns as f64) * 2.0 / 3.0).round() as u128;
            if elapsed < min_warmup_ns && next_reps <= MAX_BENCHMARK_REPETITIONS {
                self.benchmark_set(state_id, "warmup_state", Value::Int(next_state.into()))?;
                self.benchmark_set(state_id, "warmup_reps", Value::Int(next_reps.into()))?;
                self.benchmark_set(state_id, "warmup_rep_idx", Value::Int(0.into()))?;
                return self.launch_benchmark_repetition(this_id, state_id, kind);
            }
            // Warmup iteration finished; retain its computed repetition count.
            let final_reps = next_reps.clamp(1, MAX_BENCHMARK_REPETITIONS);
            self.benchmark_set(state_id, "measure_reps", Value::Int(final_reps.into()))?;
            let warmup_iter = self.benchmark_get_i64(state_id, "warmup_iter")? + 1;
            if warmup_iter < iterations {
                let now = self.benchmark_now_ns(state_id)?;
                self.benchmark_set(state_id, "warmup_iter", Value::Int(warmup_iter.into()))?;
                self.benchmark_set(state_id, "warmup_state", Value::Int(1.into()))?;
                self.benchmark_set(state_id, "warmup_reps", Value::Int(1.into()))?;
                self.benchmark_set(state_id, "warmup_rep_idx", Value::Int(0.into()))?;
                self.benchmark_set_u128(state_id, "warmup_start", now)?;
                return self.launch_benchmark_repetition(this_id, state_id, kind);
            }
            // Transition to measurement phase.
            let now = self.benchmark_now_ns(state_id)?;
            self.benchmark_set(state_id, "phase", Value::Int(PHASE_MEASURE_STEP.into()))?;
            self.benchmark_set(state_id, "measure_iter", Value::Int(0.into()))?;
            self.benchmark_set(state_id, "measure_rep_idx", Value::Int(0.into()))?;
            self.benchmark_set_u128(state_id, "measure_start", now)?;
            return self.launch_benchmark_repetition(this_id, state_id, kind);
        }

        // Measurement phase.
        let measure_reps = self.benchmark_get_i64(state_id, "measure_reps")?;
        let rep_idx = self.benchmark_get_i64(state_id, "measure_rep_idx")? + 1;
        if rep_idx < measure_reps {
            self.benchmark_set(state_id, "measure_rep_idx", Value::Int(rep_idx.into()))?;
            return self.launch_benchmark_repetition(this_id, state_id, kind);
        }
        let measure_start = self.benchmark_get_u128(state_id, "measure_start")?;
        let elapsed_ns = self
            .benchmark_now_ns(state_id)?
            .saturating_sub(measure_start)
            .max(1) as f64;
        let sample = elapsed_ns / (measure_reps as f64);
        let measure_iter = self.benchmark_get_i64(state_id, "measure_iter")?;
        self.benchmark_set(
            state_id,
            &format!("s{measure_iter}"),
            Value::Float(sample.into()),
        )?;
        let next_iter = measure_iter + 1;
        if next_iter < iterations {
            let now = self.benchmark_now_ns(state_id)?;
            self.benchmark_set(state_id, "measure_iter", Value::Int(next_iter.into()))?;
            self.benchmark_set(state_id, "measure_rep_idx", Value::Int(0.into()))?;
            self.benchmark_set_u128(state_id, "measure_start", now)?;
            return self.launch_benchmark_repetition(this_id, state_id, kind);
        }
        self.benchmark_set(state_id, "phase", Value::Int(PHASE_FINISH.into()))?;
        self.frames.last_mut().unwrap().pc -= 1;
        Ok(())
    }

    pub(super) fn benchmark_run(&mut self) -> Result<(), RuntimeError> {
        let base = self
            .frames
            .last()
            .ok_or_else(|| invalid("benchmark run frame"))?
            .base;
        let this_id = self
            .frames
            .last()
            .and_then(|f| f.environment.this_value.as_ref())
            .and_then(|v| v.heap_id())
            .ok_or_else(|| invalid("benchmark run receiver"))?;

        // Initial invocation: inspect receiver class, validate Microbenchmark
        // non-constness, allocate GC-rooted state object on the operand stack,
        // and lazily force workload properties in upstream order.
        if self.stack.len() == base {
            let kind = self.benchmark_kind(this_id)?;
            if kind == KIND_MICRO {
                self.check_microbenchmark_non_constant(this_id)?;
            }
            let state_id = self.heap.allocate(Object::default());
            self.stack.push(Value::Object(state_id));
            self.benchmark_set(state_id, "kind", Value::Int(kind.into()))?;
            self.benchmark_set(state_id, "out_stage", Value::Int(OUT_STAGE_IDLE.into()))?;
            self.frames.last_mut().unwrap().pc -= 1;
            match kind {
                KIND_OUTPUT => {
                    self.benchmark_set(
                        state_id,
                        "phase",
                        Value::Int(PHASE_READ_SOURCE_MODULE.into()),
                    )?;
                    self.force(this_id, this_id, "sourceModule".into(), false, true, false)?;
                }
                KIND_PARSER => {
                    self.benchmark_set(
                        state_id,
                        "phase",
                        Value::Int(PHASE_READ_SOURCE_TEXT.into()),
                    )?;
                    self.force(this_id, this_id, "sourceText".into(), false, true, false)?;
                }
                _ => {
                    self.benchmark_set(
                        state_id,
                        "phase",
                        Value::Int(PHASE_READ_ITERATIONS.into()),
                    )?;
                    self.force(this_id, this_id, "iterations".into(), false, true, false)?;
                }
            }
            return Ok(());
        }

        let state_id = self.stack[base].as_object()?;
        let child_result = if self.stack.len() > base + 1 {
            Some(self.pop()?)
        } else {
            None
        };
        let kind = self.benchmark_get_i64(state_id, "kind")?;
        let phase = self.benchmark_get_i64(state_id, "phase")?;

        match phase {
            PHASE_READ_SOURCE_MODULE => {
                let value = child_result.ok_or_else(|| invalid("missing sourceModule result"))?;
                let src_mod_id = value.as_object()?;
                self.benchmark_set(state_id, "source_module", Value::Object(src_mod_id))?;
                self.benchmark_set(state_id, "phase", Value::Int(PHASE_READ_ITERATIONS.into()))?;
                self.frames.last_mut().unwrap().pc -= 1;
                self.force(this_id, this_id, "iterations".into(), false, true, false)?;
                Ok(())
            }
            PHASE_READ_SOURCE_TEXT => {
                let Value::String(text) =
                    child_result.ok_or_else(|| invalid("missing sourceText result"))?
                else {
                    return Err(invalid("checked ParserBenchmark sourceText"));
                };
                self.benchmark_set(state_id, "source_text", Value::String(text))?;
                self.benchmark_set(state_id, "phase", Value::Int(PHASE_READ_SOURCE_URI.into()))?;
                self.frames.last_mut().unwrap().pc -= 1;
                self.force(this_id, this_id, "sourceUri".into(), false, true, false)?;
                Ok(())
            }
            PHASE_READ_SOURCE_URI => {
                let Value::String(uri) =
                    child_result.ok_or_else(|| invalid("missing sourceUri result"))?
                else {
                    return Err(invalid("checked ParserBenchmark sourceUri"));
                };
                if uri != "repl:text" {
                    crate::host::resources::validate_module_key_uri(&uri)?;
                }
                self.benchmark_set(state_id, "source_uri", Value::String(uri))?;
                self.benchmark_set(state_id, "phase", Value::Int(PHASE_READ_ITERATIONS.into()))?;
                self.frames.last_mut().unwrap().pc -= 1;
                self.force(this_id, this_id, "iterations".into(), false, true, false)?;
                Ok(())
            }
            PHASE_READ_ITERATIONS => {
                let Value::Int(iterations) =
                    child_result.ok_or_else(|| invalid("missing iterations result"))?
                else {
                    return Err(invalid("checked Benchmark iterations"));
                };
                self.benchmark_set(state_id, "iterations", Value::Int(iterations))?;
                self.benchmark_set(
                    state_id,
                    "phase",
                    Value::Int(PHASE_READ_ITERATION_TIME.into()),
                )?;
                self.frames.last_mut().unwrap().pc -= 1;
                self.force(this_id, this_id, "iterationTime".into(), false, true, false)?;
                Ok(())
            }
            PHASE_READ_ITERATION_TIME => {
                let Value::Duration(duration) =
                    child_result.ok_or_else(|| invalid("missing iterationTime result"))?
                else {
                    return Err(invalid("checked Benchmark iterationTime"));
                };
                self.benchmark_set(
                    state_id,
                    "iter_ns",
                    Value::Int(duration_to_nanos(duration).into()),
                )?;
                self.benchmark_set(state_id, "phase", Value::Int(PHASE_READ_IS_VERBOSE.into()))?;
                self.frames.last_mut().unwrap().pc -= 1;
                self.force(this_id, this_id, "isVerbose".into(), false, true, false)?;
                Ok(())
            }
            PHASE_READ_IS_VERBOSE => {
                let Value::Bool(verbose) =
                    child_result.ok_or_else(|| invalid("missing isVerbose result"))?
                else {
                    return Err(invalid("checked Benchmark isVerbose"));
                };
                self.benchmark_set(state_id, "verbose", Value::Bool(verbose))?;
                let iterations = self.benchmark_get_i64(state_id, "iterations")?;
                if iterations <= 0 {
                    self.benchmark_set(state_id, "measure_reps", Value::Int(0.into()))?;
                    self.benchmark_set(state_id, "phase", Value::Int(PHASE_FINISH.into()))?;
                    self.frames.last_mut().unwrap().pc -= 1;
                    return Ok(());
                }
                let now = self.benchmark_now_ns(state_id)?;
                self.benchmark_set(state_id, "phase", Value::Int(PHASE_WARMUP_STEP.into()))?;
                self.benchmark_set(state_id, "warmup_iter", Value::Int(0.into()))?;
                self.benchmark_set(state_id, "warmup_state", Value::Int(1.into()))?;
                self.benchmark_set(state_id, "warmup_reps", Value::Int(1.into()))?;
                self.benchmark_set(state_id, "warmup_rep_idx", Value::Int(0.into()))?;
                self.benchmark_set_u128(state_id, "warmup_start", now)?;
                self.launch_benchmark_repetition(this_id, state_id, kind)
            }
            PHASE_WARMUP_STEP | PHASE_MEASURE_STEP => {
                if kind == KIND_OUTPUT {
                    let out_stage = self.benchmark_get_i64(state_id, "out_stage")?;
                    if out_stage == OUT_STAGE_RELOADED {
                        let Some(Value::Object(src_mod_id)) =
                            self.benchmark_get(state_id, "source_module")?
                        else {
                            return Err(invalid("missing OutputBenchmark sourceModule"));
                        };
                        self.restore_output_benchmark_caches(state_id, src_mod_id)?;
                        let fresh_mod = child_result
                            .ok_or_else(|| invalid("missing reloaded OutputBenchmark module"))?
                            .as_object()?;
                        self.benchmark_set(
                            state_id,
                            "out_stage",
                            Value::Int(OUT_STAGE_OUTPUT.into()),
                        )?;
                        self.frames.last_mut().unwrap().pc -= 1;
                        self.force(fresh_mod, fresh_mod, "output".into(), false, true, false)?;
                        return Ok(());
                    }
                    if out_stage == OUT_STAGE_OUTPUT {
                        let output_obj = child_result
                            .ok_or_else(|| invalid("missing OutputBenchmark output"))?
                            .as_object()?;
                        let memoize = self
                            .benchmark_get(state_id, "source_module")?
                            .and_then(|v| v.heap_id())
                            .and_then(|id| self.heap.get(id).ok())
                            .is_some_and(|obj| obj.module_unit.is_some());
                        self.benchmark_set(
                            state_id,
                            "out_stage",
                            Value::Int(OUT_STAGE_TEXT.into()),
                        )?;
                        self.frames.last_mut().unwrap().pc -= 1;
                        self.force(output_obj, output_obj, "text".into(), false, memoize, false)?;
                        return Ok(());
                    }
                    self.benchmark_set(state_id, "out_stage", Value::Int(OUT_STAGE_IDLE.into()))?;
                }
                self.finish_benchmark_repetition(this_id, state_id, kind, phase)
            }
            PHASE_FINISH => {
                // Ensure `pkl:Benchmark` is cooperatively initialized before reading `BenchmarkResult`.
                let benchmark_module = match self.modules_stdlib.get("pkl:Benchmark").cloned() {
                    Some(modules::CachedModule::Value(id)) => id,
                    Some(modules::CachedModule::Failed(error)) => return Err(error),
                    None => {
                        self.frames.last_mut().unwrap().pc -= 1;
                        self.import_stdlib("pkl:Benchmark")?;
                        return Ok(());
                    }
                };
                let iterations = self.benchmark_get_i64(state_id, "iterations")?.max(0) as usize;
                let repetitions = self.benchmark_get_i64(state_id, "measure_reps")?.max(0);
                let is_verbose = matches!(
                    self.benchmark_get(state_id, "verbose")?,
                    Some(Value::Bool(true))
                );

                let mut samples_nanos = Vec::with_capacity(iterations);
                let mut min_sample = f64::MAX;
                let mut max_sample = f64::MIN_POSITIVE;
                let mut mean = 0.0f64;
                let mut sum_of_squares = 0.0f64;

                for i in 0..iterations {
                    let sample = match self.benchmark_get(state_id, &format!("s{i}"))? {
                        Some(Value::Float(v)) => v.get(),
                        _ => return Err(invalid("missing benchmark sample")),
                    };
                    samples_nanos.push(sample);
                    if sample < min_sample {
                        min_sample = sample;
                    }
                    if sample > max_sample {
                        max_sample = sample;
                    }
                    let delta = sample - mean;
                    mean += delta / ((i + 1) as f64);
                    sum_of_squares += delta * (sample - mean);
                }
                if iterations == 0 {
                    min_sample = 0.0;
                    max_sample = 0.0;
                }
                // Match BenchmarkUtils.java:74-77 (iterations == 1 yields NaN stdev/error).
                let variance = sum_of_squares / ((iterations as f64) - 1.0);
                let std_dev = variance.sqrt();
                let std_error = std_dev / (iterations as f64).sqrt();
                let error_margin = std_error * 2.576;

                let (unit, unit_factor) = choose_benchmark_unit(mean);
                let to_dur = |nanos: f64| {
                    Value::Duration(crate::duration::Duration::new(nanos / unit_factor, unit))
                };

                let samples_value = if is_verbose {
                    let list_builder = self.heap.allocate(Object {
                        collection_kind: CollectionKind::List,
                        ..Default::default()
                    });
                    self.frames.last_mut().unwrap().builders.push(list_builder);
                    for sample in &samples_nanos {
                        self.stack.push(to_dur(*sample));
                        self.append_strict_element()?;
                    }
                    self.frames.last_mut().unwrap().builders.pop();
                    Value::Object(self.finish_strict_collection(list_builder)?)
                } else {
                    Value::Null
                };

                let result_class =
                    self.get_or_create_module_class(benchmark_module, "BenchmarkResult")?;
                let result_obj = self.instantiate_typed(
                    result_class,
                    vec![
                        ("iterations", Value::Int((iterations as i64).into())),
                        ("repetitions", Value::Int(repetitions.into())),
                        ("samples", samples_value),
                        ("min", to_dur(min_sample)),
                        ("max", to_dur(max_sample)),
                        ("mean", to_dur(mean)),
                        ("stdev", to_dur(std_dev)),
                        ("error", to_dur(error_margin)),
                    ],
                )?;

                // Pop our private state object and leave only `BenchmarkResult` on stack.
                let _ = self.pop()?;
                self.stack.push(Value::Object(result_obj));
                Ok(())
            }
            _ => Err(invalid("invalid benchmark continuation phase")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn output_benchmark_restores_absent_module_cache_slots_without_leaking_fresh_module() {
        let source = r#"import "pkl:Benchmark"
x = 1 + 2
hidden ok = let (r = new Benchmark.OutputBenchmark {
  sourceModule = module
  iterations = 2
  iterationTime = 1.ms
  isVerbose = false
}.run()) r.iterations == 2 && r.repetitions >= 1
hidden ok2 = let (r = new Benchmark.OutputBenchmark {
  sourceModule = module
  iterations = 2
  iterationTime = 1.ms
  isVerbose = false
}.run()) r.iterations == 2 && r.repetitions >= 1
"#;
        let uri: PklString = "file:///m3-platform-bench/uncached-primary.pkl".into();
        let parsed = crate::parser::parse_module(source).unwrap();
        let resolved = crate::resolver::resolve_module(&parsed).unwrap();
        let mut program = compile_module(&resolved).unwrap();
        let runtime_source = Arc::new(RuntimeSource {
            text: parsed.source.clone(),
            uri: uri.clone(),
            module_name: "uncached_primary".into(),
        });
        Arc::make_mut(&mut program)
            .register_primary_source(
                runtime_source.clone(),
                crate::modules::ModuleIdentity {
                    original_uri: uri.clone(),
                    resolved_uri: uri.clone(),
                },
            )
            .unwrap();
        let mut vm = Vm::with_context(
            program.clone(),
            EvaluationContext {
                primary_source: Some(runtime_source),
                platform_metadata: Some(crate::logging::PlatformMetadata::default()),
                ..Default::default()
            },
        );
        let tick = Arc::new(AtomicU64::new(0));
        let clock_tick = tick.clone();
        vm.set_monotonic_clock(Arc::new(move || {
            u128::from(clock_tick.fetch_add(1, Ordering::Relaxed) + 1) * 1_000_000
        }));

        vm.start(program.entry, vec![]).unwrap();
        let primary = loop {
            match vm.poll(64) {
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                VmPoll::Module(req) => {
                    vm.complete_module(req.id, crate::modules::default_completion(&req).unwrap())
                        .unwrap();
                }
                VmPoll::Ready(val) => break val.unwrap().as_object().unwrap(),
                other => panic!("unexpected VM event: {other:?}"),
            }
        };

        // First, benchmark while the primary module IS cached: exact `ObjectId`
        // must be restored (not replaced with a temporary reloaded `fresh_mod`).
        assert!(matches!(
            vm.modules_original.get(uri.clone()),
            Some(modules::CachedModule::Value(id)) if *id == primary
        ));
        assert!(matches!(
            vm.modules_resolved.get(uri.clone()),
            Some(modules::CachedModule::Value(id)) if *id == primary
        ));
        vm.start_member(primary, "ok").unwrap();
        let ok = loop {
            match vm.poll(32) {
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                VmPoll::Module(req) => {
                    vm.complete_module(req.id, crate::modules::default_completion(&req).unwrap())
                        .unwrap();
                }
                VmPoll::Pending(req) => {
                    let result = crate::host::resources::default_result(&req);
                    vm.resume(req.id, result).unwrap();
                }
                VmPoll::Ready(val) => break val.unwrap(),
                other => panic!("unexpected VM event: {other:?}"),
            }
        };
        assert_eq!(ok, Value::Bool(true));
        assert!(matches!(
            vm.modules_original.get(uri.clone()),
            Some(modules::CachedModule::Value(id)) if *id == primary
        ));
        assert!(matches!(
            vm.modules_resolved.get(uri.clone()),
            Some(modules::CachedModule::Value(id)) if *id == primary
        ));

        // Second, simulate an uncached module (absent from both module caches
        // before benchmarking): previously-absent slots must stay absent.
        vm.modules_original.remove(uri.clone());
        vm.modules_resolved.remove(uri.clone());
        assert!(vm.modules_original.get(uri.clone()).is_none());
        assert!(vm.modules_resolved.get(uri.clone()).is_none());

        vm.start_member(primary, "ok2").unwrap();
        let ok2 = loop {
            match vm.poll(32) {
                VmPoll::Yielded => {
                    vm.collect(&[]);
                }
                VmPoll::Module(req) => {
                    vm.complete_module(req.id, crate::modules::default_completion(&req).unwrap())
                        .unwrap();
                }
                VmPoll::Pending(req) => {
                    let result = crate::host::resources::default_result(&req);
                    vm.resume(req.id, result).unwrap();
                }
                VmPoll::Ready(val) => break val.unwrap(),
                other => panic!("unexpected VM event: {other:?}"),
            }
        };
        assert_eq!(ok2, Value::Bool(true));
        // Previously-absent cache slots must remain absent (no stale reloaded module retained).
        assert!(vm.modules_original.get(uri.clone()).is_none());
        assert!(vm.modules_resolved.get(uri.clone()).is_none());
        assert!(vm.modules_stdlib.get(uri).is_none());

        vm.close();
        assert_eq!(vm.heap_stats().slots, 0);
    }
}
