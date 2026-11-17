//! Platform host module data and typed instantiation.
use super::*;
use crate::heap::CollectionKind;

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
        let id = self.heap.allocate(Object {
            class_definition: Some(Class {
                origin_uri,
                module: class
                    .module
                    .or_else(|| Some(self.current_module_name().into())),
                ..class
            }),
            class_initializing: true,
            declaration_code: Some(member.code),
            environment,
            ..Object::default()
        });
        let class_id = self.heap.allocate_class(id);
        let parent = self.canonical_builtin_class(classes::BuiltinClass::Typed);
        self.heap.link_superclass(class_id, Some(parent))?;
        if let Some(members) = self.program.objects.get(template) {
            self.heap.get_mut(id)?.members = members.clone();
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

        let os_name = platform_meta.os_name.as_str().unwrap_or("macOS");
        let os_version = platform_meta.os_version.as_str().unwrap_or("unknown");
        let os_class = self.get_or_create_module_class(module_id, "OperatingSystem")?;
        let os = self.instantiate_typed(
            os_class,
            vec![
                ("name", Value::string(os_name)),
                ("version", Value::string(os_version)),
            ],
        )?;

        let arch_name = platform_meta.cpu_architecture.as_str().unwrap_or("aarch64");
        let proc_class = self.get_or_create_module_class(module_id, "Processor")?;
        let proc =
            self.instantiate_typed(proc_class, vec![("architecture", Value::string(arch_name))])?;

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

    pub(super) fn benchmark_run(&mut self) -> Result<(), RuntimeError> {
        let this_id = self
            .frames
            .last()
            .and_then(|f| f.environment.this_value.as_ref())
            .and_then(|v| v.heap_id())
            .ok_or_else(|| invalid("benchmark run receiver"))?;

        let read_prop = |name: &str| -> Option<Value> {
            let mut current_id = Some(this_id);
            while let Some(id) = current_id {
                let obj = self.heap.get(id).ok()?;
                for (k, v) in &obj.member_values {
                    if k.name == name {
                        return Some(v.clone());
                    }
                }
                for (k, v) in &obj.cache {
                    if k.name == name
                        && let LazyState::Completed(val) = v
                    {
                        return Some(val.clone());
                    }
                }
                for m in &obj.members {
                    if m.name == name
                        && let Some(block) = self.program.blocks.get(m.code)
                    {
                        for inst in block {
                            if let Instruction::Constant(val) = inst {
                                return Some(val.clone());
                            }
                        }
                    }
                }
                current_id = obj.parent;
            }
            None
        };

        let iterations = match read_prop("iterations") {
            Some(Value::Int(i)) => (i.get().max(1)) as u32,
            _ => 15,
        };
        let is_verbose = match read_prop("isVerbose") {
            Some(Value::Bool(b)) => b,
            _ => false,
        };
        let iteration_time_nanos: u128 = match read_prop("iterationTime") {
            Some(Value::Duration(d)) => {
                let factor = match d.unit() {
                    crate::duration::DurationUnit::Nanoseconds => 1.0,
                    crate::duration::DurationUnit::Microseconds => 1_000.0,
                    crate::duration::DurationUnit::Milliseconds => 1_000_000.0,
                    crate::duration::DurationUnit::Seconds => 1_000_000_000.0,
                    crate::duration::DurationUnit::Minutes => 60_000_000_000.0,
                    crate::duration::DurationUnit::Hours => 3_600_000_000_000.0,
                    crate::duration::DurationUnit::Days => 86_400_000_000_000.0,
                };
                (d.value() * factor).max(1.0) as u128
            }
            _ => 100_000_000,
        };

        let source_text = match read_prop("sourceText") {
            Some(Value::String(s)) => Some(s),
            _ => None,
        };
        let source_module = match read_prop("sourceModule") {
            Some(Value::Object(id)) => Some(id),
            _ => None,
        };

        let expression_member = {
            let mut current_id = Some(this_id);
            let mut found = None;
            while let Some(id) = current_id {
                if let Ok(obj) = self.heap.get(id) {
                    for m in &obj.members {
                        if m.name == "expression" {
                            found = Some((id, m.clone()));
                            break;
                        }
                    }
                    if found.is_some() {
                        break;
                    }
                    current_id = obj.parent;
                } else {
                    break;
                }
            }
            found
        };

        let execute_workload = |vm: &mut Self| -> Result<(), RuntimeError> {
            if let Some(text) = &source_text {
                if let Some(s) = text.as_str() {
                    let _ = crate::parser::parse_module(s);
                }
            } else if let Some(module_id) = source_module {
                let _ = collections::ordered_members(&vm.heap, module_id);
                let _ = crate::diagnostic_values::render_string(
                    &Value::Object(module_id),
                    &vm.heap,
                    &vm.program.module_name,
                    80,
                );
            } else if let Some((owner_id, m)) = &expression_member
                && m.code < vm.program.blocks.len()
            {
                let target_depth = vm.frames.len();
                let mut env = Environment::default();
                env.scopes.push(Scope {
                    receiver: *owner_id,
                    owner: *owner_id,
                });
                env.this_value = Some(Value::Object(*owner_id));
                vm.frames.push(Frame {
                    guest_catch: None,
                    block: m.code,
                    pc: 0,
                    base: vm.stack.len(),
                    environment: env,
                    force: None,
                    lambda: false,
                    type_run: None,
                    type_resolution: None,
                    generic_default: None,
                    equality_run: None,
                    deep_force: None,
                    warning: None,
                    method_call_site: None,
                    pcf_render: None,
                    json_parse: None,
                    yaml_parse: None,
                    collection_join: None,
                    collection_transform: None,
                    set_order: None,
                    set_sequence: None,
                    list_ops: None,
                    string_transform: None,
                    int_seq_run: None,
                    lazy_collection_run: None,
                    mapping_lookup: None,
                    mapping_keys: None,
                    reflection: None,
                    analysis: None,
                    conversion_run: None,
                    collection_addition: None,
                    function_owner: None,
                    amendment_arguments: None,
                    amendment_end: None,
                    prepared_hashes: None,
                    key_search: None,
                    return_to_type: false,
                    diagnostic_member: None,
                    callsite_hidden: false,
                    builders: Vec::new(),
                    generator_scopes: Vec::new(),
                    module_load: None,
                    uri_resolution: None,
                    glob: None,
                    module_receiver: None,
                });
                while vm.frames.len() > target_depth {
                    vm.step()?;
                }
                let _ = vm.stack.pop();
            }
            Ok(())
        };

        let clock = self.monotonic_clock.clone();
        let mut synthetic_clock = 0u128;
        let mut now_ns = move || -> u128 {
            if let Some(ref c) = clock {
                c()
            } else {
                synthetic_clock = synthetic_clock.saturating_add(iteration_time_nanos / 2);
                synthetic_clock
            }
        };

        // Warmup loop (Knuth/Pkl benchmark protocol)
        let min_warmup_ns = ((iteration_time_nanos as f64) * 2.0 / 3.0) as u128;
        let mut repetitions = 1u64;
        let mut state = 1;
        let warmup_start = now_ns();
        loop {
            for _ in 0..repetitions {
                execute_workload(self)?;
            }
            if now_ns().saturating_sub(warmup_start) >= min_warmup_ns {
                break;
            }
            if state == 1 {
                state = 2;
                repetitions = repetitions.saturating_mul(2);
            } else if state == 2 {
                state = 5;
                repetitions = repetitions.saturating_mul(5) / 2;
            } else {
                state = 1;
                repetitions = repetitions.saturating_mul(2);
            }
            if repetitions == 0 {
                repetitions = 1;
            }
            if repetitions > 1_000_000 {
                break;
            }
        }
        if repetitions == 0 {
            repetitions = 1;
        }

        // Measurement loop
        let count = iterations as usize;
        let mut samples_nanos = Vec::with_capacity(count);
        let mut min_sample = f64::MAX;
        let mut max_sample = 0.0f64;
        let mut mean = 0.0f64;
        let mut sum_of_squares = 0.0f64;

        for i in 0..count {
            let start = now_ns();
            for _ in 0..repetitions {
                execute_workload(self)?;
            }
            let elapsed_ns = (now_ns().saturating_sub(start).max(1)) as f64;
            let sample = elapsed_ns / (repetitions as f64);
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

        let variance = if count > 1 {
            sum_of_squares / ((count - 1) as f64)
        } else {
            0.0
        };
        let std_dev = variance.sqrt();
        let std_error = std_dev / (count as f64).sqrt();
        let error_margin = std_error * 2.576;

        let unit = if mean < 1_000.0 {
            crate::duration::DurationUnit::Nanoseconds
        } else if mean < 1_000_000.0 {
            crate::duration::DurationUnit::Microseconds
        } else if mean < 1_000_000_000.0 {
            crate::duration::DurationUnit::Milliseconds
        } else {
            crate::duration::DurationUnit::Seconds
        };

        let unit_factor = match unit {
            crate::duration::DurationUnit::Nanoseconds => 1.0,
            crate::duration::DurationUnit::Microseconds => 1_000.0,
            crate::duration::DurationUnit::Milliseconds => 1_000_000.0,
            crate::duration::DurationUnit::Seconds => 1_000_000_000.0,
            _ => 1_000_000.0,
        };

        let to_dur =
            |nanos: f64| Value::Duration(crate::duration::Duration::new(nanos / unit_factor, unit));

        let min_dur = to_dur(min_sample);
        let max_dur = to_dur(max_sample);
        let mean_dur = to_dur(mean);
        let stdev_dur = to_dur(std_dev);
        let error_dur = to_dur(error_margin);

        let benchmark_module = match self.modules_stdlib.get("pkl:Benchmark") {
            Some(modules::CachedModule::Value(id)) => *id,
            _ => {
                self.import_stdlib("pkl:Benchmark")?;
                match self.modules_stdlib.get("pkl:Benchmark") {
                    Some(modules::CachedModule::Value(id)) => *id,
                    _ => return Err(invalid("missing pkl:Benchmark module")),
                }
            }
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

        let result_class = self.get_or_create_module_class(benchmark_module, "BenchmarkResult")?;
        let result_obj = self.instantiate_typed(
            result_class,
            vec![
                ("iterations", Value::Int((iterations as i64).into())),
                ("repetitions", Value::Int((repetitions as i64).into())),
                ("samples", samples_value),
                ("min", min_dur),
                ("max", max_dur),
                ("mean", mean_dur),
                ("stdev", stdev_dur),
                ("error", error_dur),
            ],
        )?;

        self.stack.push(Value::Object(result_obj));
        Ok(())
    }
}
