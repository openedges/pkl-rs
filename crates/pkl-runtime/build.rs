use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;

fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn put_varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 0x80 {
        out.push(((value & 0x7f) as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

fn pack_unicode_character_names(out_dir: &Path) {
    println!("cargo:rerun-if-changed=src/java_regex/name-index.bin");
    println!("cargo:rerun-if-changed=src/java_regex/name-pool.bin");
    println!("cargo:rerun-if-changed=src/java_regex/named-codepoints.bin");

    let index = fs::read("src/java_regex/name-index.bin").expect("read name-index.bin");
    let pool = fs::read("src/java_regex/name-pool.bin").expect("read name-pool.bin");
    let codepoints =
        fs::read("src/java_regex/named-codepoints.bin").expect("read named-codepoints.bin");

    assert_eq!(index.len() % 12, 0, "name-index.bin record alignment");
    let count = index.len() / 12;
    let mut packed_names = Vec::with_capacity(400_000);
    put_varint(&mut packed_names, u32::try_from(count).expect("count u32"));
    let mut prev: &[u8] = b"";
    for i in 0..count {
        let p = i * 12;
        let off = u32::from_be_bytes(index[p..p + 4].try_into().unwrap()) as usize;
        let len = u32::from_be_bytes(index[p + 4..p + 8].try_into().unwrap()) as usize;
        let cp = u32::from_be_bytes(index[p + 8..p + 12].try_into().unwrap());
        let name = &pool[off..off + len];
        let prefix = prev
            .iter()
            .zip(name.iter())
            .take(255)
            .take_while(|(a, b)| a == b)
            .count();
        let suffix = &name[prefix..];
        packed_names.push(prefix as u8);
        put_varint(
            &mut packed_names,
            u32::try_from(suffix.len()).expect("suffix len u32"),
        );
        packed_names.extend_from_slice(suffix);
        put_varint(&mut packed_names, cp);
        prev = name;
    }
    fs::write(out_dir.join("character_names_packed.bin"), &packed_names)
        .expect("write character_names_packed.bin");

    assert_eq!(
        codepoints.len() % 4,
        0,
        "named-codepoints.bin record alignment"
    );
    let cp_count = codepoints.len() / 4;
    let mut values = Vec::with_capacity(cp_count);
    for &chunk in codepoints.as_chunks::<4>().0 {
        values.push(u32::from_be_bytes(chunk));
    }
    let mut runs: Vec<(u32, u32)> = Vec::new();
    let mut i = 0;
    while i < values.len() {
        let start = values[i];
        let mut j = i + 1;
        while j < values.len() && values[j] == values[j - 1] + 1 {
            j += 1;
        }
        runs.push((start, (j - i) as u32));
        i = j;
    }
    let mut packed_runs = Vec::with_capacity(runs.len() * 4);
    put_varint(
        &mut packed_runs,
        u32::try_from(runs.len()).expect("run count u32"),
    );
    let mut prev_end = 0u32;
    for (start, len) in runs {
        let gap = start
            .checked_sub(prev_end)
            .expect("named-codepoints.bin must be strictly increasing");
        put_varint(&mut packed_runs, gap);
        put_varint(&mut packed_runs, len);
        prev_end = start + len;
    }
    fs::write(out_dir.join("named_codepoints_rle.bin"), &packed_runs)
        .expect("write named_codepoints_rle.bin");
}

fn main() {
    println!("cargo:rerun-if-changed=src/stdlib/base.pkl");
    println!("cargo:rerun-if-env-changed=CARGO_CFG_TARGET_ARCH");

    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    let out_dir_path = Path::new(&out_dir);
    pack_unicode_character_names(out_dir_path);
    let out_path = out_dir_path.join("base_pkl_index.rs");

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
