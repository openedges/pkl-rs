//! Self-contained, pure safe Rust read-only ZIP/JAR archive reader for `modulepath:`
//! and `jar:file:` resolution (supports STORE and RFC 1951 DEFLATE plus executable-JAR
//! preamble offsets).

use std::collections::BTreeMap;

/// Parsed read-only ZIP archive mapping normalized relative paths (no leading `/`) to bytes.
#[derive(Clone, Debug)]
pub struct ZipReader {
    entries: BTreeMap<String, Vec<u8>>,
}

impl ZipReader {
    pub fn parse(data: &[u8]) -> Result<Self, String> {
        let eocd_pos =
            find_eocd(data).ok_or_else(|| "Invalid ZIP: missing EOCD record".to_string())?;
        if eocd_pos + 22 > data.len() {
            return Err("Invalid ZIP: truncated EOCD record".to_string());
        }
        let total_entries = read_u16(data, eocd_pos + 10) as usize;
        let cd_size = read_u32(data, eocd_pos + 12) as usize;
        let cd_offset_declared = read_u32(data, eocd_pos + 16) as usize;

        // Support executable JARs with a shell preamble before the embedded ZIP bytes:
        // actual CD sits at `eocd_pos - cd_size`, so preamble offset is `actual_cd - declared_cd`.
        let actual_cd_offset = eocd_pos
            .checked_sub(cd_size)
            .ok_or_else(|| "Invalid ZIP: central directory size exceeds file length".to_string())?;
        let base_offset = actual_cd_offset.saturating_sub(cd_offset_declared);

        let mut entries = BTreeMap::new();
        let mut cursor = actual_cd_offset;
        for _ in 0..total_entries {
            if cursor + 46 > eocd_pos {
                return Err("Invalid ZIP: truncated central directory entry".to_string());
            }
            if &data[cursor..cursor + 4] != b"PK\x01\x02" {
                return Err("Invalid ZIP: bad central directory signature".to_string());
            }
            let method = read_u16(data, cursor + 10);
            let expected_crc = read_u32(data, cursor + 16);
            let comp_size = read_u32(data, cursor + 20) as usize;
            let uncomp_size = read_u32(data, cursor + 24) as usize;
            let name_len = read_u16(data, cursor + 28) as usize;
            let extra_len = read_u16(data, cursor + 30) as usize;
            let comment_len = read_u16(data, cursor + 32) as usize;
            let local_offset = read_u32(data, cursor + 42) as usize + base_offset;

            let name_start = cursor + 46;
            let name_end = name_start + name_len;
            if name_end + extra_len + comment_len > eocd_pos {
                return Err("Invalid ZIP: central directory entry overflows".to_string());
            }
            let raw_name = String::from_utf8_lossy(&data[name_start..name_end]).into_owned();
            cursor = name_end + extra_len + comment_len;

            if raw_name.ends_with('/') {
                continue;
            }

            if local_offset + 30 > data.len()
                || &data[local_offset..local_offset + 4] != b"PK\x03\x04"
            {
                return Err(format!(
                    "Invalid ZIP: bad local file header for `{raw_name}`"
                ));
            }
            let lf_name_len = read_u16(data, local_offset + 26) as usize;
            let lf_extra_len = read_u16(data, local_offset + 28) as usize;
            let payload_start = local_offset + 30 + lf_name_len + lf_extra_len;
            let payload_end = payload_start
                .checked_add(comp_size)
                .ok_or_else(|| "Invalid ZIP: entry size overflow".to_string())?;
            if payload_end > data.len() {
                return Err(format!("Invalid ZIP: truncated payload for `{raw_name}`"));
            }
            let compressed = &data[payload_start..payload_end];
            let decoded = match method {
                0 => compressed.to_vec(),
                8 => inflate_raw(compressed)?,
                other => {
                    return Err(format!(
                        "Unsupported ZIP compression method `{other}` for `{raw_name}`"
                    ));
                }
            };
            if decoded.len() != uncomp_size {
                return Err(format!(
                    "Invalid ZIP: uncompressed size mismatch for `{raw_name}`"
                ));
            }
            if crc32(&decoded) != expected_crc {
                return Err(format!("Invalid ZIP: CRC-32 mismatch for `{raw_name}`"));
            }
            let normalized = raw_name.trim_start_matches('/').to_string();
            if !normalized.is_empty() {
                entries.insert(normalized, decoded);
            }
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> impl Iterator<Item = (&str, &[u8])> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        let normalized = path.trim_start_matches('/');
        self.entries.get(normalized).map(Vec::as_slice)
    }

    pub fn is_directory(&self, path: &str) -> bool {
        let trimmed = path.trim_matches('/');
        if trimmed.is_empty() {
            return true;
        }
        let prefix = format!("{trimmed}/");
        self.entries.keys().any(|k| k.starts_with(&prefix))
    }
}

fn find_eocd(data: &[u8]) -> Option<usize> {
    if data.len() < 22 {
        return None;
    }
    let min_pos = data.len().saturating_sub(22 + 65535);
    let mut pos = data.len() - 22;
    loop {
        if &data[pos..pos + 4] == b"PK\x05\x06" {
            let comment_len = read_u16(data, pos + 20) as usize;
            if pos + 22 + comment_len == data.len() {
                return Some(pos);
            }
        }
        if pos == min_pos {
            break;
        }
        pos -= 1;
    }
    None
}

fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= u32::from(b);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

// --- RFC 1951 raw DEFLATE decoder (100% safe Rust) ---

struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
    buf: u64,
    bits: u32,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            buf: 0,
            bits: 0,
        }
    }

    fn ensure(&mut self, n: u32) -> Result<(), String> {
        while self.bits < n {
            if self.pos >= self.data.len() {
                return Err("Truncated DEFLATE stream".to_string());
            }
            self.buf |= u64::from(self.data[self.pos]) << self.bits;
            self.pos += 1;
            self.bits += 8;
        }
        Ok(())
    }

    fn read_bits(&mut self, n: u32) -> Result<u32, String> {
        if n == 0 {
            return Ok(0);
        }
        self.ensure(n)?;
        let val = (self.buf & ((1u64 << n) - 1)) as u32;
        self.buf >>= n;
        self.bits -= n;
        Ok(val)
    }

    fn align_byte(&mut self) {
        let drop = self.bits % 8;
        self.buf >>= drop;
        self.bits -= drop;
    }

    fn read_aligned_bytes(&mut self, len: usize) -> Result<Vec<u8>, String> {
        self.align_byte();
        let mut out = Vec::with_capacity(len);
        while self.bits >= 8 && out.len() < len {
            out.push((self.buf & 0xFF) as u8);
            self.buf >>= 8;
            self.bits -= 8;
        }
        let rem = len - out.len();
        if self.pos + rem > self.data.len() {
            return Err("Truncated stored DEFLATE block".to_string());
        }
        out.extend_from_slice(&self.data[self.pos..self.pos + rem]);
        self.pos += rem;
        Ok(out)
    }
}

struct HuffmanTree {
    counts: [u16; 16],
    symbols: Vec<u16>,
    max_bits: usize,
}

impl HuffmanTree {
    fn from_lengths(lengths: &[u8]) -> Result<Self, String> {
        let mut counts = [0u16; 16];
        let mut max_bits = 0usize;
        for &len in lengths {
            let l = usize::from(len);
            if l > 15 {
                return Err("Invalid Huffman code length > 15".to_string());
            }
            if l > 0 {
                counts[l] = counts[l]
                    .checked_add(1)
                    .ok_or_else(|| "Huffman count overflow".to_string())?;
                if l > max_bits {
                    max_bits = l;
                }
            }
        }
        let mut offsets = [0usize; 16];
        let mut total = 0usize;
        for bits in 1..=15 {
            offsets[bits] = total;
            total += usize::from(counts[bits]);
        }
        let mut symbols = vec![0u16; total];
        for (sym, &len) in lengths.iter().enumerate() {
            let l = usize::from(len);
            if l > 0 {
                symbols[offsets[l]] = sym as u16;
                offsets[l] += 1;
            }
        }
        Ok(Self {
            counts,
            symbols,
            max_bits,
        })
    }

    fn decode(&self, reader: &mut BitReader<'_>) -> Result<u16, String> {
        let mut code = 0i32;
        let mut first = 0i32;
        let mut index = 0usize;
        for bits in 1..=self.max_bits {
            code |= reader.read_bits(1)? as i32;
            let count = i32::from(self.counts[bits]);
            if code - count < first {
                return Ok(self.symbols[index + (code - first) as usize]);
            }
            index += count as usize;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("Invalid Huffman symbol in DEFLATE stream".to_string())
    }
}

fn inflate_raw(input: &[u8]) -> Result<Vec<u8>, String> {
    const LEN_BASE: [u16; 29] = [
        3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115,
        131, 163, 195, 227, 258,
    ];
    const LEN_EXTRA: [u32; 29] = [
        0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
    ];
    const DIST_BASE: [u16; 30] = [
        1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
        2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
    ];
    const DIST_EXTRA: [u32; 30] = [
        0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12,
        13, 13,
    ];
    const CL_ORDER: [usize; 19] = [
        16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
    ];

    let mut reader = BitReader::new(input);
    let mut out = Vec::new();
    loop {
        let bfinal = reader.read_bits(1)?;
        let btype = reader.read_bits(2)?;
        match btype {
            0 => {
                let header = reader.read_aligned_bytes(4)?;
                let len = u16::from_le_bytes([header[0], header[1]]);
                let nlen = u16::from_le_bytes([header[2], header[3]]);
                if len != !nlen {
                    return Err("Invalid stored block length complement".to_string());
                }
                let block = reader.read_aligned_bytes(usize::from(len))?;
                out.extend_from_slice(&block);
            }
            1 | 2 => {
                let (lit_tree, dist_tree) = if btype == 1 {
                    let mut lit_lens = vec![0u8; 288];
                    for l in &mut lit_lens[0..144] {
                        *l = 8;
                    }
                    for l in &mut lit_lens[144..256] {
                        *l = 9;
                    }
                    for l in &mut lit_lens[256..280] {
                        *l = 7;
                    }
                    for l in &mut lit_lens[280..288] {
                        *l = 8;
                    }
                    let dist_lens = vec![5u8; 32];
                    (
                        HuffmanTree::from_lengths(&lit_lens)?,
                        HuffmanTree::from_lengths(&dist_lens)?,
                    )
                } else {
                    let hlit = reader.read_bits(5)? as usize + 257;
                    let hdist = reader.read_bits(5)? as usize + 1;
                    let hclen = reader.read_bits(4)? as usize + 4;
                    let mut cl_lens = [0u8; 19];
                    for &idx in &CL_ORDER[..hclen] {
                        cl_lens[idx] = reader.read_bits(3)? as u8;
                    }
                    let cl_tree = HuffmanTree::from_lengths(&cl_lens)?;
                    let total = hlit + hdist;
                    let mut lens = Vec::with_capacity(total);
                    while lens.len() < total {
                        let sym = cl_tree.decode(&mut reader)?;
                        match sym {
                            0..=15 => lens.push(sym as u8),
                            16 => {
                                let prev = *lens
                                    .last()
                                    .ok_or_else(|| "Invalid repeat code 16 at start".to_string())?;
                                let repeat = reader.read_bits(2)? as usize + 3;
                                for _ in 0..repeat {
                                    lens.push(prev);
                                }
                            }
                            17 => {
                                let repeat = reader.read_bits(3)? as usize + 3;
                                lens.resize(lens.len() + repeat, 0);
                            }
                            18 => {
                                let repeat = reader.read_bits(7)? as usize + 11;
                                lens.resize(lens.len() + repeat, 0);
                            }
                            _ => return Err("Invalid code-length symbol".to_string()),
                        }
                    }
                    if lens.len() != total {
                        return Err("Huffman code-length table overflow".to_string());
                    }
                    (
                        HuffmanTree::from_lengths(&lens[..hlit])?,
                        HuffmanTree::from_lengths(&lens[hlit..])?,
                    )
                };

                loop {
                    let sym = lit_tree.decode(&mut reader)?;
                    if sym < 256 {
                        out.push(sym as u8);
                    } else if sym == 256 {
                        break;
                    } else if sym <= 285 {
                        let len_idx = usize::from(sym - 257);
                        let length = usize::from(LEN_BASE[len_idx])
                            + reader.read_bits(LEN_EXTRA[len_idx])? as usize;
                        let dist_sym = dist_tree.decode(&mut reader)? as usize;
                        if dist_sym >= 30 {
                            return Err("Invalid distance symbol in DEFLATE stream".to_string());
                        }
                        let dist = usize::from(DIST_BASE[dist_sym])
                            + reader.read_bits(DIST_EXTRA[dist_sym])? as usize;
                        if dist == 0 || dist > out.len() {
                            return Err("DEFLATE back-reference before start of output".to_string());
                        }
                        for _ in 0..length {
                            let b = out[out.len() - dist];
                            out.push(b);
                        }
                    } else {
                        return Err("Invalid literal/length symbol in DEFLATE stream".to_string());
                    }
                }
            }
            _ => return Err("Invalid DEFLATE block type 3".to_string()),
        }
        if bfinal == 1 {
            break;
        }
    }
    Ok(out)
}
