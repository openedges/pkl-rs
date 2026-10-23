//! Development-only source-language annotation/Host comparison adapter.
use pkl_runtime::evaluation::session::{SourceSession, SourceSessionOptions};
use pkl_runtime::host::HostPoll;
use pkl_runtime::value::{RuntimeError, Value};
use std::{fs, path::Path};

const URI: &str = "file:///m2-platform.pkl";
const NAME: &str = "m2-platform";

fn expressions(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .expect("read expressions")
        .lines()
        .map(str::to_owned)
        .collect()
}
fn event(session: &mut SourceSession, collections: &mut usize) -> HostPoll {
    for _ in 0..100_000 {
        let event = session.poll(0, 32);
        let roots = match &event {
            HostPoll::Ready(Ok(value)) => vec![value.clone()],
            _ => Vec::new(),
        };
        session.collect(&roots);
        *collections += 1;
        if !matches!(event, HostPoll::Yielded) {
            return event;
        }
    }
    panic!("annotation Host probe instruction budget exceeded");
}
fn capture(out: &Path, prefix: &str, session: &SourceSession, result: Result<Value, RuntimeError>) {
    let (status, stdout, stderr) = match result {
        Ok(value) => (
            "success\n",
            SourceSession::result_string(value)
                .expect("expression string")
                .to_java_utf8(),
            String::new(),
        ),
        Err(error) => ("error\n", String::new(), session.format_error(error)),
    };
    for (channel, bytes) in [
        ("status", status.as_bytes()),
        ("stdout", stdout.as_bytes()),
        ("stderr", stderr.as_bytes()),
        ("error-class", b""),
    ] {
        fs::write(out.join(format!("{prefix}.{channel}.bin")), bytes).expect("write call capture");
    }
}
struct Probe<'a> {
    session: SourceSession,
    collections: usize,
    reads: usize,
    out: &'a Path,
    levels: Vec<Vec<usize>>,
}
impl Probe<'_> {
    fn evaluate(&mut self, depth: usize) -> Result<Value, RuntimeError> {
        loop {
            match event(&mut self.session, &mut self.collections) {
                HostPoll::Ready(result) => return result,
                HostPoll::Read { token, uri } => {
                    self.reads += 1;
                    let call = self.reads;
                    if self.levels.len() > 1 {
                        println!("host-read={uri} calls={call} depth={}", depth + 1);
                    } else {
                        println!("host-read={uri} calls={call}");
                    }
                    let callbacks = self
                        .levels
                        .get(depth)
                        .expect("configured callback depth")
                        .clone();
                    for (index, entry) in callbacks.into_iter().enumerate() {
                        self.session.reenter_path(&token, entry, 0, None).unwrap();
                        let result = self.evaluate(depth + 1);
                        capture(
                            self.out,
                            &format!("inner-{call}-{index}"),
                            &self.session,
                            result,
                        );
                        assert_eq!(self.session.context_depth(), depth + 1);
                    }
                    assert!(
                        matches!(event(&mut self.session, &mut self.collections), HostPoll::Read { token: pending, .. } if pending == token)
                    );
                    self.session
                        .resume(&token, Ok(Value::string("resource")), 0)
                        .unwrap();
                }
                HostPoll::Log { .. } => {
                    unreachable!("default annotation adapter uses the no-op logger")
                }
                HostPoll::Module { .. } => {
                    unreachable!("fixed annotation adapter has no module Host capability")
                }
                HostPoll::Yielded => unreachable!(),
            }
        }
    }
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert!(
        args.len() >= 5,
        "source outer-expressions inner-expressions output [deeper-inner-expressions ...]"
    );
    let text = fs::read_to_string(&args[1]).expect("read source");
    let outer = expressions(Path::new(&args[2]));
    let mut inner_levels = vec![expressions(Path::new(&args[3]))];
    inner_levels.extend(args[5..].iter().map(|path| expressions(Path::new(path))));
    let out = Path::new(&args[4]);
    let paths: Vec<_> = outer
        .iter()
        .chain(inner_levels.iter().flatten())
        .cloned()
        .collect();
    let session = SourceSession::compile(&text, URI, NAME, &paths, SourceSessionOptions::default())
        .expect("compile authored source");
    let mut next = outer.len();
    let levels = inner_levels
        .into_iter()
        .map(|paths| {
            paths
                .into_iter()
                .map(|_| {
                    let index = next;
                    next += 1;
                    index
                })
                .collect()
        })
        .collect();
    let mut probe = Probe {
        session,
        collections: 0,
        reads: 0,
        out,
        levels,
    };
    for index in 0..outer.len() {
        probe.session.start_path(index, 0, None).unwrap();
        let result = probe.evaluate(0);
        capture(out, &format!("outer-{index}"), &probe.session, result);
        assert_eq!(probe.session.context_depth(), 0);
        println!("attempt={index} calls={}", probe.reads);
    }
    probe.session.close();
    let stats = probe.session.heap_stats();
    assert_eq!(stats.slots, 0, "terminal close releases the entire heap");
    fs::write(
        out.join("architecture.json"),
        format!(
            "{{\"collections\":{},\"reads\":{},\"terminal_slots\":{}}}\n",
            probe.collections, probe.reads, stats.slots
        ),
    )
    .expect("write architecture observations");
}
