//! Development-only raw-value differential probe; not a product CLI.
use pkl_runtime::parser::parse_module;
use pkl_runtime::resolver::resolve_module;
use pkl_runtime::value::Value;
use pkl_runtime::vm::{Vm, VmPoll, compile_module};

fn run() -> Result<String, String> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 3 {
        return Err("expected source path and member name".into());
    }
    let source = std::fs::read_to_string(&arguments[1]).map_err(|error| error.to_string())?;
    let module = parse_module(&source).map_err(|error| format!("{error:?}"))?;
    let resolved = resolve_module(&module).map_err(|error| format!("{error:?}"))?;
    let program = compile_module(&resolved).map_err(|error| error.to_string())?;
    let mut vm = Vm::new(program.clone());
    vm.start(program.entry, vec![])
        .map_err(|error| error.to_string())?;
    let object = poll(&mut vm)?
        .as_object()
        .map_err(|error| error.to_string())?;
    vm.start_member(object, &arguments[2])
        .map_err(|error| error.to_string())?;
    match poll(&mut vm)? {
        Value::Int(value) => Ok(value.to_string()),
        Value::Float(value) => Ok(pkl_runtime::numeric::double_to_string(value)),
        Value::Bool(value) => Ok(value.to_string()),
        Value::String(value) => Ok(value.to_string()),
        value => Err(format!("unsupported probe value {value:?}")),
    }
}

fn poll(vm: &mut Vm) -> Result<Value, String> {
    for _ in 0..1000 {
        match vm.poll(1024) {
            VmPoll::Ready(result) => return result.map_err(|error| error.to_string()),
            VmPoll::Yielded => {}
            VmPoll::Pending(_) => return Err("probe does not supply a Host".into()),
        }
    }
    Err("probe instruction limit exceeded".into())
}

fn main() {
    match run() {
        Ok(value) => print!("{value}"),
        Err(error) => {
            eprint!("{error}");
            std::process::exit(1);
        }
    }
}
