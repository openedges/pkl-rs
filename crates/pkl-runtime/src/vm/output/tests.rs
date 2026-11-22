//! Structural graph assertions for the actual Module prototype, not Java heap counts.
use super::*;

fn compiled(source: &str) -> Arc<Program> {
    let parsed = crate::parser::parse_module(source).unwrap();
    compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap()
}

fn ready(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..10_000 {
        match vm.poll(1) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {
                vm.collect(&[]);
            }
            other => panic!("unexpected prototype graph event: {other:?}"),
        }
    }
    panic!("prototype graph evaluation did not finish");
}

#[test]
fn captured_module_keeps_exactly_one_real_parent_until_terminal_release() {
    let program = compiled(
        "open class Base { fixed x: Int = 1 }\nclass Child extends Base { early: Int = 2; x = 3; late = 4 }\nscalar = 7\n",
    );
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap().as_object().unwrap();
    let parent = vm.heap.get(module).unwrap().parent.unwrap();
    assert_eq!(
        vm.heap
            .get(module)
            .unwrap()
            .class_definition
            .as_ref()
            .unwrap()
            .id,
        reference::MODULE_ID
    );
    let builtin = vm.heap.get(parent).unwrap();
    assert_eq!(builtin.class_definition.as_ref().unwrap().id, MODULE);
    assert!(
        builtin
            .builtin_type
            .as_ref()
            .unwrap()
            .equivalent(&builtins::ty(MODULE))
    );
    assert_eq!(
        vm.heap
            .get(builtin.parent.unwrap())
            .unwrap()
            .class_definition
            .as_ref()
            .unwrap()
            .name,
        "Typed"
    );
    assert!(builtin.cache.is_empty());
    assert!(builtin.members.iter().any(|member| member.name == "output"));
    assert_eq!(
        vm.member_state(module, "output").unwrap(),
        LazyState::Unevaluated
    );

    // The private base bootstrap and its renderer call targets are evaluator
    // roots. This test counts only the two new user descriptor/prototype pairs beyond that
    // established graph, and still checks the exact four user handles below.
    let initialized_live = vm.collect(&[]).live;
    vm.start_member(module, "Child").unwrap();
    assert_eq!(ready(&mut vm).unwrap_err().kind, ErrorKind::Type);
    assert_eq!(vm.class_singletons.len(), 2);
    let classes = vm.class_singletons.values().copied().collect::<Vec<_>>();
    let base = classes
        .iter()
        .copied()
        .find(|id| vm.heap.class_definition(*id).unwrap().name == "Base")
        .unwrap();
    let child = classes
        .iter()
        .copied()
        .find(|id| vm.heap.class_definition(*id).unwrap().name == "Child")
        .unwrap();
    let typed = vm.canonical_builtin_class(classes::BuiltinClass::Typed);
    assert_eq!(vm.heap.get_class(base).unwrap().superclass, Some(typed));
    assert_eq!(vm.heap.get_class(child).unwrap().superclass, Some(base));
    for class in [base, child] {
        let scope = vm
            .heap
            .get(vm.heap.get_class(class).unwrap().prototype)
            .unwrap()
            .environment
            .scopes
            .last()
            .unwrap();
        assert_eq!((scope.receiver, scope.owner), (module, module));
    }
    assert_eq!(
        [module, parent, base, child]
            .into_iter()
            .collect::<BTreeSet<_>>()
            .len(),
        4
    );
    assert_eq!(vm.collect(&[]).live, initialized_live + 4);
    vm.start_member(module, "scalar").unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::Int(7.into())));
    assert_eq!(vm.collect(&[]).live, initialized_live + 4);
    assert!(vm.heap.get(parent).unwrap().cache.is_empty());
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
    assert!(vm.class_singletons.is_empty());
    for id in [module, parent, base, child] {
        assert_eq!(vm.heap.get(id).unwrap_err().kind, ErrorKind::StaleHandle);
    }
}

#[test]
fn unrooted_module_is_reclaimed_while_base_keeps_the_shared_parent() {
    let program = compiled("scalar = 7\n");
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![]).unwrap();
    let module = ready(&mut vm).unwrap().as_object().unwrap();
    let parent = vm.heap.get(module).unwrap().parent.unwrap();
    let initialized_live = vm.collect(&[]).live;
    vm.start_member(module, "scalar").unwrap();
    assert_eq!(ready(&mut vm), Ok(Value::Int(7.into())));
    assert!(vm.class_singletons.is_empty());
    // The initialized base module roots the same canonical Module prototype.
    // Releasing the user module and its Class descriptor keeps that shared parent.
    let base = match vm.modules_stdlib.get("pkl:base").unwrap() {
        modules::CachedModule::Value(id) => *id,
        _ => panic!("base was initialized successfully"),
    };
    assert_eq!(vm.heap.get(base).unwrap().parent, Some(parent));
    assert_eq!(vm.collect(&[]).live, initialized_live - 2);
    assert_eq!(
        vm.heap.get(module).unwrap_err().kind,
        ErrorKind::StaleHandle
    );
    assert!(vm.heap.get(parent).unwrap().cache.is_empty());
    vm.start(program.entry, vec![]).unwrap();
    let replacement = ready(&mut vm).unwrap().as_object().unwrap();
    let replacement_parent = vm.heap.get(replacement).unwrap().parent.unwrap();
    assert_ne!(replacement, module);
    assert_eq!(replacement_parent, parent);
    assert_eq!(vm.collect(&[]).live, initialized_live);
    vm.close();
    assert_eq!(vm.heap_stats().live, 0);
    assert_eq!(vm.heap_stats().slots, 0);
    for id in [module, parent, base, replacement] {
        assert_eq!(vm.heap.get(id).unwrap_err().kind, ErrorKind::StaleHandle);
    }
}

fn vm_for_source(source: &str) -> (Vm, Arc<Program>) {
    let parsed = crate::parser::parse_module(source).unwrap();
    let program = compile_module(&crate::resolver::resolve_module(&parsed).unwrap()).unwrap();
    let vm = Vm::with_context(
        program.clone(),
        EvaluationContext {
            primary_source: Some(Arc::new(crate::value::RuntimeSource {
                text: parsed.source,
                uri: "file:///output-fast-test.pkl".into(),
                module_name: "output_fast_test".into(),
            })),
            ..Default::default()
        },
    );
    (vm, program)
}

fn ready_with_stdlib(vm: &mut Vm) -> Result<Value, RuntimeError> {
    for _ in 0..100_000 {
        match vm.poll(64) {
            VmPoll::Ready(result) => return result,
            VmPoll::Yielded => {}
            VmPoll::Module(request) => {
                let completion = crate::modules::default_completion(&request).unwrap();
                vm.complete_module(request.id, completion).unwrap();
            }
            other => panic!("unexpected render test event: {other:?}"),
        }
    }
    panic!("render test evaluation did not finish");
}

fn eval_module_member(source: &str, name: &str) -> Result<Value, RuntimeError> {
    let (mut vm, program) = vm_for_source(source);
    vm.start(program.entry, vec![]).unwrap();
    let module = ready_with_stdlib(&mut vm)?.as_object().unwrap();
    vm.start_member(module, name).unwrap();
    let res = ready_with_stdlib(&mut vm);
    vm.close();
    res
}

#[test]
fn fast_renderers_match_converter_fallback_across_all_eight_formats() {
    let source = r#"
import "pkl:xml"
import "pkl:jsonnet"
import "pkl:pklbinary"

items = IntSeq(1, 12).map((i) -> new Dynamic {
  id = i
  name = "svc-\(i)"
  label = "항목-\(i)-😀"
  enabled = i % 2 == 0
  ratio = i / 10.0
  tags = List("tier-\(i % 4)", "zone-\(i % 3)")
})
payload = new Dynamic {
  version = "0.32.0"
  count = items.length
  entries = items
}
binItems = IntSeq(1, 12).map((i) -> new Dynamic {
  id = i
  key = "item-\(i)"
  big = 9007199254740993 + i
  raw = Bytes(i % 256, (i * 7) % 256, 255, 0)
  duration = i.ms
  size = i.kib
  tags = Set("a", "b-\(i % 5)")
})
binPayload = new Dynamic {
  entries = binItems
  meta = new Dynamic {
    left = Pair("k", 42)
    right = Map("x", 1, "y", 2)
  }
}
flatProps = new Dynamic {
  for (i in IntSeq(1, 30)) {
    ["svc_\(i)_name"] = "service-\(i)"
    ["svc_\(i)_port"] = "\(8000 + i)"
  }
}
local noop = new Mapping { ["__never__"] = (v) -> v }

pcfFast = (new PcfRenderer {}).renderDocument(payload)
pcfSlow = (new PcfRenderer { converters = noop }).renderDocument(payload)
jsonFast = (new JsonRenderer {}).renderDocument(payload)
jsonSlow = (new JsonRenderer { converters = noop }).renderDocument(payload)
yamlFast = (new YamlRenderer {}).renderDocument(payload)
yamlSlow = (new YamlRenderer { converters = noop }).renderDocument(payload)
xmlFast = (new xml.Renderer {}).renderDocument(payload)
xmlSlow = (new xml.Renderer { converters = noop }).renderDocument(payload)
propsFast = (new PropertiesRenderer {}).renderDocument(flatProps)
propsSlow = (new PropertiesRenderer { converters = noop }).renderDocument(flatProps)
plistFast = (new PListRenderer {}).renderDocument(payload)
plistSlow = (new PListRenderer { converters = noop }).renderDocument(payload)
jsonnetFast = (new jsonnet.Renderer {}).renderDocument(payload)
jsonnetSlow = (new jsonnet.Renderer { converters = noop }).renderDocument(payload)
binFast = (new pklbinary.Renderer {}).renderDocument(binPayload)
binSlow = (new pklbinary.Renderer { converters = noop }).renderDocument(binPayload)
"#;
    let (mut vm, program) = vm_for_source(source);
    vm.start(program.entry, vec![]).unwrap();
    let module = ready_with_stdlib(&mut vm).unwrap().as_object().unwrap();

    // Plain Dynamic items without generator scopes must not duplicate environments per member.
    vm.start_member(module, "items").unwrap();
    let items_id = ready_with_stdlib(&mut vm).unwrap().as_object().unwrap();
    let first_item = vm
        .heap
        .get(items_id)
        .unwrap()
        .list_elements
        .as_ref()
        .unwrap()[0]
        .as_object()
        .unwrap();
    assert!(
        vm.heap
            .get(first_item)
            .unwrap()
            .member_environments
            .is_empty()
    );

    for (fast, slow) in [
        ("pcfFast", "pcfSlow"),
        ("jsonFast", "jsonSlow"),
        ("yamlFast", "yamlSlow"),
        ("xmlFast", "xmlSlow"),
        ("propsFast", "propsSlow"),
        ("plistFast", "plistSlow"),
        ("jsonnetFast", "jsonnetSlow"),
        ("binFast", "binSlow"),
    ] {
        vm.start_member(module, fast).unwrap();
        let fast_val = ready_with_stdlib(&mut vm).unwrap();
        vm.start_member(module, slow).unwrap();
        let slow_val = ready_with_stdlib(&mut vm).unwrap();
        assert_eq!(fast_val, slow_val, "fast vs slow mismatch on {fast}");
    }
    vm.close();
}

#[test]
fn fast_list_undefined_property_preserves_render_path_hint() {
    let source = r#"
class Item {
  id: Int
  missing: String
}
local items = List(
  new Dynamic { id = 1; missing = "ok" },
  new Item { id = 2 },
)
payload = new Dynamic { entries = items }
rendered = (new JsonRenderer {}).renderDocument(payload)
"#;
    let err = eval_module_member(source, "rendered").unwrap_err();
    assert_eq!(err.kind, ErrorKind::MissingMember);
    assert!(
        err.hint
            .as_ref()
            .and_then(|h| h.as_str())
            .is_some_and(|h| h.contains("entries[1].missing")),
        "unexpected hint: {:?}",
        err.hint
    );
}

#[test]
fn fast_render_cycle_falls_back_cleanly_to_cycle_error() {
    let source = r#"
import "pkl:pklbinary"
cyclic = new Dynamic {
  selfRef = cyclic
}
pcf = (new PcfRenderer {}).renderDocument(cyclic)
json = (new JsonRenderer {}).renderDocument(cyclic)
yaml = (new YamlRenderer {}).renderDocument(cyclic)
bin = (new pklbinary.Renderer {}).renderDocument(cyclic)
"#;
    for member in ["pcf", "json", "yaml", "bin"] {
        let err = eval_module_member(source, member).unwrap_err();
        assert_eq!(err.kind, ErrorKind::Cycle, "expected cycle on {member}");
    }
}

#[test]
fn sync_cache_probe_handles_rehash_growth_and_object_entry_keys() {
    // 270 string entries exercise both >4 hash table creation and >256 grow_before_append rehash.
    let source = r#"
big = new Dynamic {
  for (i in IntSeq(1, 270)) {
    ["k_\(i)"] = i * 3
  }
}
bigCheck = (new PropertiesRenderer {}).renderDocument(big)
bigLookup = big["k_1"] + big["k_135"] + big["k_270"]

mixed = new Dynamic {
  ["a"] = 1
  ["b"] = 2
  ["c"] = 3
  ["d"] = 4
  [new Dynamic { x = 1 }] = 10
  ["e"] = 5
  [new Dynamic { x = 2 }] = 20
}
mixedLookup = mixed["a"] + mixed["e"] + mixed[new Dynamic { x = 1 }] + mixed[new Dynamic { x = 2 }]
"#;
    assert_eq!(
        eval_module_member(source, "bigLookup"),
        Ok(Value::Int((3 + 405 + 810).into()))
    );
    let rendered = eval_module_member(source, "bigCheck").unwrap();
    let Value::String(text) = rendered else {
        panic!("expected string");
    };
    let s = text.as_str().unwrap();
    assert!(s.contains("k_1 = 3\n"));
    assert!(s.contains("k_270 = 810\n"));
    assert_eq!(
        eval_module_member(source, "mixedLookup"),
        Ok(Value::Int(36.into()))
    );
}
