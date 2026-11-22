use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn main() {
    println!("cargo:rerun-if-changed=src/stdlib/base.pkl");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let out_path = Path::new(&out_dir).join("base_pkl_index.rs");

    if env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        fs::write(
            &out_path,
            "// Empty on wasm32 to avoid static binary size growth.\n",
        )
        .expect("write empty base_pkl_index.rs");
        return;
    }

    let bytes = fs::read("src/stdlib/base.pkl").expect("read src/stdlib/base.pkl");
    assert!(
        bytes.len() <= u32::MAX as usize,
        "base.pkl exceeds u32 offset range"
    );

    let mut close_braces: Vec<u32> = Vec::new();
    for (i, win) in bytes.windows(2).enumerate() {
        if win == b"\n}" {
            close_braces.push(i as u32);
        }
    }

    let mut postings: BTreeMap<&[u8], Vec<u32>> = BTreeMap::new();
    let mut i = 0;
    while i < bytes.len() {
        if is_word_byte(bytes[i]) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_word_byte(bytes[i]) {
                i += 1;
            }
            postings
                .entry(&bytes[start..i])
                .or_default()
                .push(start as u32);
        } else {
            i += 1;
        }
    }

    let mut flat_offsets: Vec<u32> = Vec::new();
    let mut word_entries: Vec<(u32, u16, u16, u16)> = Vec::with_capacity(postings.len());
    for (word, offs) in &postings {
        let post_start = u16::try_from(flat_offsets.len()).expect("postings start fits u16");
        let post_len = u16::try_from(offs.len()).expect("postings len fits u16");
        let word_len = u16::try_from(word.len()).expect("word len fits u16");
        let sample_off = offs[0];
        word_entries.push((sample_off, word_len, post_start, post_len));
        flat_offsets.extend_from_slice(offs);
    }

    let mut file = fs::File::create(&out_path).expect("create base_pkl_index.rs");
    writeln!(
        file,
        "pub(super) const BASE_CLOSE_BRACE_OFFSETS: &[u32] = &["
    )
    .unwrap();
    for chunk in close_braces.chunks(16) {
        write!(file, "    ").unwrap();
        for off in chunk {
            write!(file, "{off}, ").unwrap();
        }
        writeln!(file).unwrap();
    }
    writeln!(file, "];").unwrap();

    writeln!(
        file,
        "pub(super) const BASE_WORDS: &[(u32, u16, u16, u16)] = &["
    )
    .unwrap();
    for chunk in word_entries.chunks(6) {
        write!(file, "    ").unwrap();
        for (sample_off, word_len, post_start, post_len) in chunk {
            write!(
                file,
                "({sample_off}, {word_len}, {post_start}, {post_len}), "
            )
            .unwrap();
        }
        writeln!(file).unwrap();
    }
    writeln!(file, "];").unwrap();

    writeln!(file, "pub(super) const BASE_WORD_OFFSETS: &[u32] = &[").unwrap();
    for chunk in flat_offsets.chunks(16) {
        write!(file, "    ").unwrap();
        for off in chunk {
            write!(file, "{off}, ").unwrap();
        }
        writeln!(file).unwrap();
    }
    writeln!(file, "];").unwrap();
}
