//! Safe in-memory ZIP reader and DEFLATE (RFC 1951) decompressor.
use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug)]
pub enum ZipError {
    InvalidSignature,
    UnexpectedEof,
    UnsupportedCompression(u16),
    CorruptedData(&'static str),
}

impl fmt::Display for ZipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSignature => write!(f, "Invalid ZIP signature"),
            Self::UnexpectedEof => write!(f, "Unexpected EOF while parsing ZIP"),
            Self::UnsupportedCompression(c) => write!(f, "Unsupported compression method: {c}"),
            Self::CorruptedData(msg) => write!(f, "Corrupted ZIP data: {msg}"),
        }
    }
}

impl std::error::Error for ZipError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ZipReadError {
    NotFound,
    IsADirectory,
}

pub fn percent_decode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut i = 0;
    let s_bytes = s.as_bytes();
    while i < s_bytes.len() {
        if s_bytes[i] == b'%'
            && i + 2 < s_bytes.len()
            && let Ok(byte) = u8::from_str_radix(
                std::str::from_utf8(&s_bytes[i + 1..i + 3]).unwrap_or_default(),
                16,
            )
        {
            bytes.push(byte);
            i += 3;
            continue;
        }
        bytes.push(s_bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

pub fn percent_encode_fragment(fragment: &str) -> String {
    let mut encoded = String::with_capacity(fragment.len());
    let bytes = fragment.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%'
            && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            encoded.push('%');
            encoded.push(bytes[i + 1] as char);
            encoded.push(bytes[i + 2] as char);
            i += 3;
            continue;
        }
        if b.is_ascii_alphanumeric() || b"-_.!~*'();/:@&=+$,?[]".contains(&b) {
            encoded.push(b as char);
        } else {
            use std::fmt::Write;
            write!(&mut encoded, "%{b:02X}").unwrap();
        }
        i += 1;
    }
    encoded
}

pub fn normalize_zip_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        if seg.is_empty() || seg == "." {
            continue;
        } else if seg == ".." {
            parts.pop();
        } else {
            parts.push(seg);
        }
    }
    if parts.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", parts.join("/"))
    }
}

const MAX_ZIP_ENTRY_UNCOMPRESSED_BYTES: usize = 128 * 1024 * 1024;
const MAX_ZIP_TOTAL_UNCOMPRESSED_BYTES: usize = 256 * 1024 * 1024;
const MAX_ZIP_INITIAL_ALLOC_BYTES: usize = 64 * 1024;
const MAX_ZIP_PATH_BYTES: usize = 8192;
const MAX_ZIP_PATH_DEPTH: usize = 2048;
const MAX_ZIP_DIR_METADATA_BYTES: usize = 16 * 1024 * 1024;

fn validate_zip_entry_path(filename: &str) -> Result<(), ZipError> {
    if filename.len() > MAX_ZIP_PATH_BYTES {
        return Err(ZipError::CorruptedData(
            "ZIP entry path exceeds maximum allowed byte length",
        ));
    }
    if filename.contains('\0') || filename.contains('\\') {
        return Err(ZipError::CorruptedData(
            "ZIP entry path contains forbidden NUL or backslash character",
        ));
    }
    let mut depth = 0usize;
    for seg in filename.split('/') {
        if seg == ".." {
            return Err(ZipError::CorruptedData(
                "ZIP entry path contains forbidden parent-directory traversal (`..`)",
            ));
        }
        if !seg.is_empty() {
            depth += 1;
            if depth > MAX_ZIP_PATH_DEPTH {
                return Err(ZipError::CorruptedData(
                    "ZIP entry path exceeds maximum directory nesting depth",
                ));
            }
        }
    }
    Ok(())
}

/// Represents an in-memory ZIP archive with normalized entry paths.
#[derive(Clone, Debug, Default)]
pub struct ZipArchive {
    /// Maps normalized asset path (e.g. "/catalog/Swallow.pkl") to uncompressed bytes.
    pub entries: BTreeMap<String, Vec<u8>>,
    /// Set of directory paths (e.g. "/catalog", "/").
    pub directories: BTreeMap<String, Vec<String>>,
    /// Per-entry `(dos_time, dos_date)` recorded from the central directory.
    pub dos_mtimes: BTreeMap<String, (u16, u16)>,
}

impl ZipArchive {
    /// Parses and strictly validates a ZIP archive from raw bytes (EOCD, central directory,
    /// local headers, compression, uncompressed size, and CRC-32).
    pub fn parse(bytes: &[u8]) -> Result<Self, ZipError> {
        let mut archive = ZipArchive::default();
        archive.directories.insert("/".into(), Vec::new());

        // Find End of Central Directory Record (EOCD) from the end
        if bytes.len() < 22 {
            return Err(ZipError::UnexpectedEof);
        }

        let eocd_pos = find_eocd(bytes)?;
        let num_entries = u16::from_le_bytes([bytes[eocd_pos + 10], bytes[eocd_pos + 11]]) as usize;
        let cd_size = u32::from_le_bytes([
            bytes[eocd_pos + 12],
            bytes[eocd_pos + 13],
            bytes[eocd_pos + 14],
            bytes[eocd_pos + 15],
        ]) as usize;
        let cd_offset = u32::from_le_bytes([
            bytes[eocd_pos + 16],
            bytes[eocd_pos + 17],
            bytes[eocd_pos + 18],
            bytes[eocd_pos + 19],
        ]) as usize;

        let Some(cd_end) = cd_offset.checked_add(cd_size) else {
            return Err(ZipError::CorruptedData("Central directory offset overflow"));
        };
        if cd_end > eocd_pos || cd_end > bytes.len() {
            return Err(ZipError::CorruptedData("Central directory out of bounds"));
        }

        let mut total_uncompressed: usize = 0;
        let mut total_dir_metadata_bytes: usize = 0;
        let mut offset = cd_offset;
        for _ in 0..num_entries {
            if offset + 46 > cd_end {
                return Err(ZipError::UnexpectedEof);
            }
            let sig = u32::from_le_bytes([
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ]);
            if sig != 0x02014b50 {
                return Err(ZipError::InvalidSignature);
            }

            let version_made_by = u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]]);
            let method = u16::from_le_bytes([bytes[offset + 10], bytes[offset + 11]]);
            let dos_time = u16::from_le_bytes([bytes[offset + 12], bytes[offset + 13]]);
            let dos_date = u16::from_le_bytes([bytes[offset + 14], bytes[offset + 15]]);
            let cd_crc32 = u32::from_le_bytes([
                bytes[offset + 16],
                bytes[offset + 17],
                bytes[offset + 18],
                bytes[offset + 19],
            ]);
            let cd_compressed_size = u32::from_le_bytes([
                bytes[offset + 20],
                bytes[offset + 21],
                bytes[offset + 22],
                bytes[offset + 23],
            ]) as usize;
            let cd_uncompressed_size = u32::from_le_bytes([
                bytes[offset + 24],
                bytes[offset + 25],
                bytes[offset + 26],
                bytes[offset + 27],
            ]) as usize;
            let filename_len =
                u16::from_le_bytes([bytes[offset + 28], bytes[offset + 29]]) as usize;
            let extra_len = u16::from_le_bytes([bytes[offset + 30], bytes[offset + 31]]) as usize;
            let comment_len = u16::from_le_bytes([bytes[offset + 32], bytes[offset + 33]]) as usize;
            let external_attrs = u32::from_le_bytes([
                bytes[offset + 38],
                bytes[offset + 39],
                bytes[offset + 40],
                bytes[offset + 41],
            ]);
            let local_header_offset = u32::from_le_bytes([
                bytes[offset + 42],
                bytes[offset + 43],
                bytes[offset + 44],
                bytes[offset + 45],
            ]) as usize;

            let filename_start = offset + 46;
            let Some(filename_end) = filename_start.checked_add(filename_len) else {
                return Err(ZipError::UnexpectedEof);
            };
            let Some(next_offset) = filename_end
                .checked_add(extra_len)
                .and_then(|n| n.checked_add(comment_len))
            else {
                return Err(ZipError::UnexpectedEof);
            };
            if next_offset > cd_end {
                return Err(ZipError::UnexpectedEof);
            }
            let filename =
                String::from_utf8_lossy(&bytes[filename_start..filename_end]).to_string();

            offset = next_offset;

            validate_zip_entry_path(&filename)?;

            // Skip Unix/macOS symlink entries (`S_IFLNK = 0o120000` when OS == 3 Unix or 19 macOS)
            // so archives cannot smuggle symlinks as regular files.
            let is_unix_symlink = matches!(version_made_by >> 8, 3 | 19)
                && ((external_attrs >> 16) & 0o170000) == 0o120000;
            if is_unix_symlink {
                continue;
            }

            // Validate and extract entry from local file header
            let Some(local_fixed_end) = local_header_offset.checked_add(30) else {
                return Err(ZipError::UnexpectedEof);
            };
            if local_fixed_end > bytes.len() {
                return Err(ZipError::UnexpectedEof);
            }
            let local_sig = u32::from_le_bytes([
                bytes[local_header_offset],
                bytes[local_header_offset + 1],
                bytes[local_header_offset + 2],
                bytes[local_header_offset + 3],
            ]);
            if local_sig != 0x04034b50 {
                return Err(ZipError::InvalidSignature);
            }
            let local_fn_len = u16::from_le_bytes([
                bytes[local_header_offset + 26],
                bytes[local_header_offset + 27],
            ]) as usize;
            let local_extra_len = u16::from_le_bytes([
                bytes[local_header_offset + 28],
                bytes[local_header_offset + 29],
            ]) as usize;
            let compressed_size = cd_compressed_size;
            let uncompressed_size = cd_uncompressed_size;

            let Some(data_start) = local_fixed_end
                .checked_add(local_fn_len)
                .and_then(|n| n.checked_add(local_extra_len))
            else {
                return Err(ZipError::UnexpectedEof);
            };
            let Some(data_end) = data_start.checked_add(compressed_size) else {
                return Err(ZipError::UnexpectedEof);
            };
            if data_end > bytes.len() {
                return Err(ZipError::UnexpectedEof);
            }

            let normalized_path = if filename.starts_with('/') {
                filename.clone()
            } else {
                format!("/{filename}")
            };

            if filename.ends_with('/') {
                // Directory entry
                let dir_path = normalized_path.trim_end_matches('/').to_string();
                let dir_key = if dir_path.is_empty() {
                    "/".to_string()
                } else {
                    dir_path
                };
                archive.add_directory_hierarchy(&dir_key, &mut total_dir_metadata_bytes)?;
            } else {
                if uncompressed_size > MAX_ZIP_ENTRY_UNCOMPRESSED_BYTES {
                    return Err(ZipError::CorruptedData(
                        "ZIP entry uncompressed size exceeds maximum limit",
                    ));
                }
                let remaining_archive_budget =
                    MAX_ZIP_TOTAL_UNCOMPRESSED_BYTES.saturating_sub(total_uncompressed);
                let entry_limit = remaining_archive_budget.min(MAX_ZIP_ENTRY_UNCOMPRESSED_BYTES);
                if uncompressed_size > entry_limit {
                    return Err(ZipError::CorruptedData(
                        "ZIP archive total uncompressed size exceeds maximum limit",
                    ));
                }

                // File entry
                let compressed_data = &bytes[data_start..data_end];
                let decompressed = match method {
                    0 => {
                        if compressed_data.len() != uncompressed_size {
                            return Err(ZipError::CorruptedData("Stored entry size mismatch"));
                        }
                        compressed_data.to_vec()
                    }
                    8 => inflate(compressed_data, uncompressed_size, entry_limit)?,
                    other => return Err(ZipError::UnsupportedCompression(other)),
                };
                if decompressed.len() != uncompressed_size {
                    return Err(ZipError::CorruptedData("Decompressed entry size mismatch"));
                }
                if compute_crc32(&decompressed) != cd_crc32 {
                    return Err(ZipError::CorruptedData("Entry CRC-32 mismatch"));
                }
                total_uncompressed = total_uncompressed.saturating_add(decompressed.len());
                archive
                    .dos_mtimes
                    .insert(normalized_path.clone(), (dos_time, dos_date));
                archive.add_file(normalized_path, decompressed, &mut total_dir_metadata_bytes)?;
            }
        }

        if offset != cd_end {
            return Err(ZipError::CorruptedData(
                "Central directory size does not match entry count",
            ));
        }

        Ok(archive)
    }

    pub fn entry_dos_mtime(&self, path: &str) -> Option<(u16, u16)> {
        let norm = normalize_zip_path(path);
        self.dos_mtimes.get(&norm).copied()
    }

    /// Iteratively ensures every ancestor directory of `dir_path` exists in `self.directories`
    /// without recursion and bounded by `MAX_ZIP_DIR_METADATA_BYTES`.
    fn add_directory_hierarchy(
        &mut self,
        dir_path: &str,
        dir_metadata_bytes: &mut usize,
    ) -> Result<(), ZipError> {
        let mut chain: Vec<&str> = Vec::new();
        let mut cur = dir_path;
        while !self.directories.contains_key(cur) {
            chain.push(cur);
            if cur == "/" {
                break;
            }
            cur = match cur.rfind('/') {
                Some(0) => "/",
                Some(idx) => &cur[..idx],
                None => "/",
            };
        }
        for path in chain.into_iter().rev() {
            *dir_metadata_bytes = dir_metadata_bytes.saturating_add(path.len() + 64);
            if *dir_metadata_bytes > MAX_ZIP_DIR_METADATA_BYTES {
                return Err(ZipError::CorruptedData(
                    "ZIP directory metadata exceeds maximum allowed size",
                ));
            }
            self.directories.entry(path.to_string()).or_default();
            if path != "/" {
                let (parent, leaf) = match path.rfind('/') {
                    Some(0) => ("/", &path[1..]),
                    Some(idx) => (&path[..idx], &path[idx + 1..]),
                    None => ("/", path),
                };
                if let Some(children) = self.directories.get_mut(parent)
                    && !children.iter().any(|c| c == leaf)
                {
                    *dir_metadata_bytes = dir_metadata_bytes.saturating_add(leaf.len() + 24);
                    if *dir_metadata_bytes > MAX_ZIP_DIR_METADATA_BYTES {
                        return Err(ZipError::CorruptedData(
                            "ZIP directory metadata exceeds maximum allowed size",
                        ));
                    }
                    children.push(leaf.to_string());
                }
            }
        }
        Ok(())
    }

    fn add_file(
        &mut self,
        normalized_path: String,
        data: Vec<u8>,
        dir_metadata_bytes: &mut usize,
    ) -> Result<(), ZipError> {
        let parent = match normalized_path.rfind('/') {
            Some(0) => "/",
            Some(idx) => &normalized_path[..idx],
            None => "/",
        };
        let leaf = normalized_path[normalized_path.rfind('/').map_or(0, |i| i + 1)..].to_string();
        self.add_directory_hierarchy(parent, dir_metadata_bytes)?;
        if let Some(children) = self.directories.get_mut(parent)
            && !children.iter().any(|c| c == &leaf)
        {
            *dir_metadata_bytes = dir_metadata_bytes.saturating_add(leaf.len() + 24);
            if *dir_metadata_bytes > MAX_ZIP_DIR_METADATA_BYTES {
                return Err(ZipError::CorruptedData(
                    "ZIP directory metadata exceeds maximum allowed size",
                ));
            }
            children.push(leaf);
        }
        self.entries.insert(normalized_path, data);
        Ok(())
    }

    pub fn is_directory(&self, path: &str) -> bool {
        let norm = normalize_zip_path(path);
        let decoded = percent_decode(&norm);
        norm == "/"
            || decoded == "/"
            || self.directories.contains_key(&norm)
            || self.directories.contains_key(&decoded)
    }

    pub fn is_file(&self, path: &str) -> bool {
        let norm = normalize_zip_path(path);
        let decoded = percent_decode(&norm);
        self.entries.contains_key(&norm) || self.entries.contains_key(&decoded)
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        let norm = normalize_zip_path(path);
        if let Some(v) = self.entries.get(&norm) {
            return Some(v.as_slice());
        }
        let decoded = percent_decode(&norm);
        self.entries.get(&decoded).map(|v| v.as_slice())
    }

    pub fn read_bytes(&self, path: &str, allow_directories: bool) -> Result<Vec<u8>, ZipReadError> {
        let norm = normalize_zip_path(path);
        let decoded = percent_decode(&norm);
        if let Some(data) = self
            .entries
            .get(&norm)
            .or_else(|| self.entries.get(&decoded))
        {
            return Ok(data.clone());
        }
        if norm == "/"
            || decoded == "/"
            || self.directories.contains_key(&norm)
            || self.directories.contains_key(&decoded)
        {
            if allow_directories {
                let lookup_dir = if self.directories.contains_key(&decoded) {
                    &decoded
                } else if self.directories.contains_key(&norm) {
                    &norm
                } else {
                    "/"
                };
                let mut children = self
                    .directories
                    .get(lookup_dir)
                    .cloned()
                    .unwrap_or_default();
                children.sort();
                let mut text = children.join("\n");
                text.push('\n');
                return Ok(text.into_bytes());
            } else {
                return Err(ZipReadError::IsADirectory);
            }
        }
        Err(ZipReadError::NotFound)
    }

    pub fn has_element(&self, path: &str) -> bool {
        let norm = normalize_zip_path(path);
        let decoded = percent_decode(&norm);
        norm == "/"
            || decoded == "/"
            || self.entries.contains_key(&norm)
            || self.entries.contains_key(&decoded)
            || self.directories.contains_key(&norm)
            || self.directories.contains_key(&decoded)
    }

    pub fn list_elements(&self, path: &str) -> Vec<(String, bool)> {
        let norm = normalize_zip_path(path);
        let decoded = percent_decode(&norm);
        let lookup_dir = if self.directories.contains_key(&decoded) {
            decoded.as_str()
        } else if self.directories.contains_key(&norm) {
            norm.as_str()
        } else if norm == "/" || decoded == "/" {
            "/"
        } else {
            return Vec::new();
        };
        if let Some(children) = self.directories.get(lookup_dir) {
            let mut result: Vec<(String, bool)> = children
                .iter()
                .map(|name| {
                    let child_path = if lookup_dir == "/" {
                        format!("/{name}")
                    } else {
                        format!("{lookup_dir}/{name}")
                    };
                    let is_dir = self.directories.contains_key(&child_path);
                    (name.clone(), is_dir)
                })
                .collect();
            result.sort_by(|a, b| a.0.cmp(&b.0));
            result
        } else {
            Vec::new()
        }
    }
}

fn find_eocd(bytes: &[u8]) -> Result<usize, ZipError> {
    let min_pos = bytes.len().saturating_sub(65536 + 22);
    for pos in (min_pos..=bytes.len() - 22).rev() {
        if bytes[pos] == 0x50
            && bytes[pos + 1] == 0x4b
            && bytes[pos + 2] == 0x05
            && bytes[pos + 3] == 0x06
        {
            let comment_len = u16::from_le_bytes([bytes[pos + 20], bytes[pos + 21]]) as usize;
            if pos + 22 + comment_len == bytes.len() {
                return Ok(pos);
            }
        }
    }
    Err(ZipError::InvalidSignature)
}

// ---------------------------------------------------------------------------
// Pure safe Rust RFC 1951 DEFLATE decompressor (Inflate)
// ---------------------------------------------------------------------------

struct BitReader<'a> {
    bytes: &'a [u8],
    bit_pos: usize,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, bit_pos: 0 }
    }

    fn peek_bits(&self, count: usize) -> u32 {
        let mut result = 0u32;
        for i in 0..count {
            let pos = self.bit_pos + i;
            let byte_idx = pos / 8;
            let bit_idx = pos % 8;
            if byte_idx < self.bytes.len() {
                let bit = (self.bytes[byte_idx] >> bit_idx) & 1;
                result |= (bit as u32) << i;
            }
        }
        result
    }

    fn consume_bits(&mut self, count: usize) {
        self.bit_pos += count;
    }

    fn read_bits(&mut self, count: usize) -> Result<u32, ZipError> {
        let mut result = 0u32;
        for i in 0..count {
            let byte_idx = self.bit_pos / 8;
            let bit_idx = self.bit_pos % 8;
            if byte_idx >= self.bytes.len() {
                return Err(ZipError::UnexpectedEof);
            }
            let bit = (self.bytes[byte_idx] >> bit_idx) & 1;
            result |= (bit as u32) << i;
            self.bit_pos += 1;
        }
        Ok(result)
    }

    fn align_byte(&mut self) {
        let rem = self.bit_pos % 8;
        if rem != 0 {
            self.bit_pos += 8 - rem;
        }
    }

    fn read_u16(&mut self) -> Result<u16, ZipError> {
        self.align_byte();
        let idx = self.bit_pos / 8;
        if idx + 2 > self.bytes.len() {
            return Err(ZipError::UnexpectedEof);
        }
        let val = u16::from_le_bytes([self.bytes[idx], self.bytes[idx + 1]]);
        self.bit_pos += 16;
        Ok(val)
    }

    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8], ZipError> {
        self.align_byte();
        let idx = self.bit_pos / 8;
        if idx + len > self.bytes.len() {
            return Err(ZipError::UnexpectedEof);
        }
        let slice = &self.bytes[idx..idx + len];
        self.bit_pos += len * 8;
        Ok(slice)
    }
}

/// Fast lookup Huffman decoder (direct array lookup for <= 9 bits)
struct FastHuffman {
    lookup: Vec<u32>, // (symbol << 8) | len
}

impl FastHuffman {
    fn from_code_lengths(lengths: &[u8]) -> Result<Self, ZipError> {
        let mut max_len = 0;
        let mut bl_count = [0u32; 16];
        for &len in lengths {
            if len > 15 {
                return Err(ZipError::CorruptedData("Code length > 15"));
            }
            if len > 0 {
                bl_count[len as usize] += 1;
                max_len = max_len.max(len as usize);
            }
        }
        if max_len == 0 {
            return Ok(Self { lookup: Vec::new() });
        }

        let mut next_code = [0u32; 16];
        let mut code = 0u32;
        for bits in 1..=15 {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }

        let table_bits = max_len;
        let table_size = 1usize << table_bits;
        let mut lookup = vec![0u32; table_size];

        for (symbol, &len) in lengths.iter().enumerate() {
            if len > 0 {
                let sym_code = next_code[len as usize];
                next_code[len as usize] += 1;
                let mut rev_code = 0usize;
                for i in 0..len {
                    rev_code |= (((sym_code >> (len - 1 - i)) & 1) as usize) << i;
                }
                let step = 1usize << len;
                let entry = ((symbol as u32) << 8) | (len as u32);
                let mut i = rev_code;
                while i < table_size {
                    lookup[i] = entry;
                    i += step;
                }
            }
        }

        Ok(Self { lookup })
    }

    fn decode(&self, reader: &mut BitReader<'_>) -> Result<u16, ZipError> {
        if self.lookup.is_empty() {
            return Err(ZipError::CorruptedData("Empty Huffman table"));
        }
        let bits_needed = self.lookup.len().trailing_zeros() as usize;
        let peek = reader.peek_bits(bits_needed);
        let entry = self.lookup[peek as usize];
        let len = (entry & 0xff) as usize;
        let symbol = (entry >> 8) as u16;
        if len == 0 || reader.bit_pos + len > reader.bytes.len() * 8 {
            return Err(ZipError::CorruptedData("Invalid Huffman code"));
        }
        reader.consume_bits(len);
        Ok(symbol)
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

const CL_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn inflate(
    compressed: &[u8],
    expected_size: usize,
    max_output_bytes: usize,
) -> Result<Vec<u8>, ZipError> {
    let mut reader = BitReader::new(compressed);
    let mut output = Vec::with_capacity(
        expected_size
            .min(max_output_bytes)
            .min(MAX_ZIP_INITIAL_ALLOC_BYTES),
    );
    let mut is_final = false;

    while !is_final {
        is_final = reader.read_bits(1)? == 1;
        let btype = reader.read_bits(2)?;

        match btype {
            0 => {
                // Stored block
                let len = reader.read_u16()? as usize;
                let nlen = reader.read_u16()? as usize;
                if len != (nlen ^ 0xffff) {
                    return Err(ZipError::CorruptedData("Stored block length mismatch"));
                }
                if output.len().saturating_add(len) > max_output_bytes {
                    return Err(ZipError::CorruptedData(
                        "ZIP entry decompressed size exceeds maximum limit",
                    ));
                }
                let bytes = reader.read_bytes(len)?;
                output
                    .try_reserve(bytes.len())
                    .map_err(|_| ZipError::CorruptedData("ZIP inflate out of memory"))?;
                output.extend_from_slice(bytes);
            }
            1 => {
                // Fixed Huffman
                let lit_tree = fixed_literal_tree()?;
                let dist_tree = fixed_distance_tree()?;
                decode_huffman_block(
                    &mut reader,
                    &lit_tree,
                    &dist_tree,
                    &mut output,
                    max_output_bytes,
                )?;
            }
            2 => {
                // Dynamic Huffman
                let hlit = (reader.read_bits(5)? + 257) as usize;
                let hdist = (reader.read_bits(5)? + 1) as usize;
                let hclen = (reader.read_bits(4)? + 4) as usize;

                let mut cl_lengths = [0u8; 19];
                for i in 0..hclen {
                    cl_lengths[CL_ORDER[i]] = reader.read_bits(3)? as u8;
                }
                let cl_tree = FastHuffman::from_code_lengths(&cl_lengths)?;

                let mut lengths = Vec::with_capacity(hlit + hdist);
                while lengths.len() < hlit + hdist {
                    let sym = cl_tree.decode(&mut reader)?;
                    match sym {
                        0..=15 => lengths.push(sym as u8),
                        16 => {
                            let repeat = (reader.read_bits(2)? + 3) as usize;
                            let prev = *lengths
                                .last()
                                .ok_or(ZipError::CorruptedData("No prev len"))?;
                            for _ in 0..repeat {
                                lengths.push(prev);
                            }
                        }
                        17 => {
                            let repeat = (reader.read_bits(3)? + 3) as usize;
                            lengths.extend(std::iter::repeat_n(0, repeat));
                        }
                        18 => {
                            let repeat = (reader.read_bits(7)? + 11) as usize;
                            lengths.extend(std::iter::repeat_n(0, repeat));
                        }
                        _ => return Err(ZipError::CorruptedData("Invalid CL symbol")),
                    }
                }
                if lengths.len() > hlit + hdist {
                    return Err(ZipError::CorruptedData(
                        "Dynamic Huffman code length overflow",
                    ));
                }

                let lit_tree = FastHuffman::from_code_lengths(&lengths[..hlit])?;
                let dist_tree = FastHuffman::from_code_lengths(&lengths[hlit..])?;
                decode_huffman_block(
                    &mut reader,
                    &lit_tree,
                    &dist_tree,
                    &mut output,
                    max_output_bytes,
                )?;
            }
            _ => return Err(ZipError::CorruptedData("Reserved block type")),
        }
    }

    Ok(output)
}

fn fixed_literal_tree() -> Result<FastHuffman, ZipError> {
    let mut lengths = [0u8; 288];
    for item in lengths.iter_mut().take(143 + 1) {
        *item = 8;
    }
    for item in lengths.iter_mut().take(255 + 1).skip(144) {
        *item = 9;
    }
    for item in lengths.iter_mut().take(279 + 1).skip(256) {
        *item = 7;
    }
    for item in lengths.iter_mut().skip(280) {
        *item = 8;
    }
    FastHuffman::from_code_lengths(&lengths)
}

fn fixed_distance_tree() -> Result<FastHuffman, ZipError> {
    let lengths = [5u8; 32];
    FastHuffman::from_code_lengths(&lengths)
}

fn decode_huffman_block(
    reader: &mut BitReader<'_>,
    lit_tree: &FastHuffman,
    dist_tree: &FastHuffman,
    output: &mut Vec<u8>,
    max_output_bytes: usize,
) -> Result<(), ZipError> {
    loop {
        let sym = lit_tree.decode(reader)?;
        match sym {
            0..=255 => {
                if output.len() >= max_output_bytes {
                    return Err(ZipError::CorruptedData(
                        "ZIP entry decompressed size exceeds maximum limit",
                    ));
                }
                output.push(sym as u8);
            }
            256 => break, // End of block
            257..=285 => {
                let len_idx = (sym - 257) as usize;
                let base_len = LENGTH_BASE[len_idx] as usize;
                let extra_bits = LENGTH_EXTRA[len_idx] as usize;
                let length = if extra_bits > 0 {
                    base_len + reader.read_bits(extra_bits)? as usize
                } else {
                    base_len
                };

                let dist_sym = dist_tree.decode(reader)? as usize;
                if dist_sym >= 30 {
                    return Err(ZipError::CorruptedData("Invalid distance symbol"));
                }
                let base_dist = DISTANCE_BASE[dist_sym] as usize;
                let dist_extra = DISTANCE_EXTRA[dist_sym] as usize;
                let distance = if dist_extra > 0 {
                    base_dist + reader.read_bits(dist_extra)? as usize
                } else {
                    base_dist
                };

                if distance > output.len() {
                    return Err(ZipError::CorruptedData("Distance exceeds output"));
                }
                if output.len().saturating_add(length) > max_output_bytes {
                    return Err(ZipError::CorruptedData(
                        "ZIP entry decompressed size exceeds maximum limit",
                    ));
                }
                output
                    .try_reserve(length)
                    .map_err(|_| ZipError::CorruptedData("ZIP inflate out of memory"))?;
                let start = output.len() - distance;
                for i in 0..length {
                    let b = output[start + i];
                    output.push(b);
                }
            }
            _ => return Err(ZipError::CorruptedData("Invalid literal/length symbol")),
        }
    }
    Ok(())
}

/// Fixed DOS mtime `1980-02-01 00:00:00` matching `ProjectPackager.ZIP_ENTRY_MTIME`.
pub const PACKAGE_ZIP_DOS_TIME: u16 = 0x0000;
/// DOS date `((1980 - 1980) << 9) | (2 << 5) | 1 = 0x0041` (`1980-02-01`).
pub const PACKAGE_ZIP_DOS_DATE: u16 = 0x0041;

/// Helper to pack entries into a deterministic RFC 1951 DEFLATE (method 8) ZIP archive
/// with fixed DOS mtime `1980-02-01 00:00:00`, UTF-8 filenames, sorted normalized relative
/// paths, and no directory entries (matching `ProjectPackager.createPackageZipAndComputeChecksum`).
///
/// Empty input emits the exact 22-byte EOCD record (`SHA-256 = 8739c76e...76d85`).
pub fn build_deterministic_package_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    try_build_deterministic_package_zip(entries).expect("ZIP32 archive bounds exceeded")
}

/// Fallible counterpart to [`build_deterministic_package_zip`] validating ZIP32 `u16`/`u32` bounds.
pub fn try_build_deterministic_package_zip(entries: &[(&str, &[u8])]) -> Result<Vec<u8>, ZipError> {
    let mut sorted: Vec<(String, &[u8])> = entries
        .iter()
        .map(|(name, bytes)| {
            let clean = name.replace('\\', "/");
            let trimmed = clean.trim_start_matches('/').to_string();
            (trimmed, *bytes)
        })
        .collect();
    sorted.sort_by(|(a, _), (b, _)| a.cmp(b));

    let num_entries = u16::try_from(sorted.len())
        .map_err(|_| ZipError::CorruptedData("ZIP entry count exceeds ZIP32 u16 limit"))?;

    let mut zip = Vec::new();
    let mut cd = Vec::new();
    const UTF8_FLAG: u16 = 0x0800;

    for (name, content) in &sorted {
        let offset = u32::try_from(zip.len())
            .map_err(|_| ZipError::CorruptedData("ZIP local header offset exceeds ZIP32 limit"))?;
        let fn_bytes = name.as_bytes();
        let fn_len = u16::try_from(fn_bytes.len())
            .map_err(|_| ZipError::CorruptedData("ZIP entry filename exceeds u16 length limit"))?;
        let uncompressed_size = u32::try_from(content.len())
            .map_err(|_| ZipError::CorruptedData("ZIP entry size exceeds ZIP32 u32 limit"))?;
        let crc = compute_crc32(content);
        let compressed = deflate_raw(content);
        let compressed_size = u32::try_from(compressed.len())
            .map_err(|_| ZipError::CorruptedData("ZIP compressed size exceeds ZIP32 u32 limit"))?;

        // Local header (30 bytes)
        zip.extend_from_slice(&0x04034b50u32.to_le_bytes());
        zip.extend_from_slice(&20u16.to_le_bytes()); // version needed (2.0 for DEFLATE)
        zip.extend_from_slice(&UTF8_FLAG.to_le_bytes());
        zip.extend_from_slice(&8u16.to_le_bytes()); // method 8 (DEFLATE)
        zip.extend_from_slice(&PACKAGE_ZIP_DOS_TIME.to_le_bytes());
        zip.extend_from_slice(&PACKAGE_ZIP_DOS_DATE.to_le_bytes());
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&compressed_size.to_le_bytes());
        zip.extend_from_slice(&uncompressed_size.to_le_bytes());
        zip.extend_from_slice(&fn_len.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes()); // extra len
        zip.extend_from_slice(fn_bytes);
        zip.extend_from_slice(&compressed);

        // Central directory entry (46 bytes)
        cd.extend_from_slice(&0x02014b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes()); // version made by
        cd.extend_from_slice(&20u16.to_le_bytes()); // version needed
        cd.extend_from_slice(&UTF8_FLAG.to_le_bytes());
        cd.extend_from_slice(&8u16.to_le_bytes()); // method 8 (DEFLATE)
        cd.extend_from_slice(&PACKAGE_ZIP_DOS_TIME.to_le_bytes());
        cd.extend_from_slice(&PACKAGE_ZIP_DOS_DATE.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&compressed_size.to_le_bytes());
        cd.extend_from_slice(&uncompressed_size.to_le_bytes());
        cd.extend_from_slice(&fn_len.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // extra len
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment len
        cd.extend_from_slice(&0u16.to_le_bytes()); // disk start
        cd.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        cd.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        cd.extend_from_slice(&offset.to_le_bytes());
        cd.extend_from_slice(fn_bytes);
    }

    let cd_offset = u32::try_from(zip.len())
        .map_err(|_| ZipError::CorruptedData("ZIP central directory offset exceeds ZIP32 limit"))?;
    let cd_size = u32::try_from(cd.len())
        .map_err(|_| ZipError::CorruptedData("ZIP central directory size exceeds ZIP32 limit"))?;

    zip.extend_from_slice(&cd);

    // EOCD (22 bytes)
    zip.extend_from_slice(&0x06054b50u32.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes()); // disk num
    zip.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
    zip.extend_from_slice(&num_entries.to_le_bytes());
    zip.extend_from_slice(&num_entries.to_le_bytes());
    zip.extend_from_slice(&cd_size.to_le_bytes());
    zip.extend_from_slice(&cd_offset.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes()); // comment len

    Ok(zip)
}

/// Deterministic pure-Rust RFC 1951 raw DEFLATE encoder (LZ77 + Fixed Huffman, with
/// automatic stored-block fallback when uncompressed DEFLATE blocks are smaller).
pub fn deflate_raw(input: &[u8]) -> Vec<u8> {
    if input.is_empty() {
        // BFINAL=1, BTYPE=01 (fixed Huffman), EOB symbol 256 (7 zero bits)
        return vec![0x03, 0x00];
    }
    let fixed = deflate_fixed_huffman(input);
    let stored_size = input.len() + input.len().div_ceil(65535) * 5;
    if fixed.len() <= stored_size {
        fixed
    } else {
        deflate_stored(input)
    }
}

fn deflate_stored(input: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(input.len() + input.len().div_ceil(65535) * 5);
    let mut offset = 0;
    while offset < input.len() {
        let chunk_len = (input.len() - offset).min(65535);
        let is_final = offset + chunk_len == input.len();
        out.push(if is_final { 0x01 } else { 0x00 }); // BFINAL, BTYPE=00
        let len = chunk_len as u16;
        let nlen = !len;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(&input[offset..offset + chunk_len]);
        offset += chunk_len;
    }
    out
}

struct BitWriter {
    out: Vec<u8>,
    bit_buf: u32,
    bit_count: u32,
}

impl BitWriter {
    fn with_capacity(cap: usize) -> Self {
        Self {
            out: Vec::with_capacity(cap),
            bit_buf: 0,
            bit_count: 0,
        }
    }

    #[inline]
    fn write_bits_lsb(&mut self, value: u32, count: u32) {
        self.bit_buf |= value << self.bit_count;
        self.bit_count += count;
        while self.bit_count >= 8 {
            self.out.push((self.bit_buf & 0xff) as u8);
            self.bit_buf >>= 8;
            self.bit_count -= 8;
        }
    }

    #[inline]
    fn write_huffman_msb(&mut self, code: u32, len: u32) {
        let rev = (code.reverse_bits()) >> (32 - len);
        self.write_bits_lsb(rev, len);
    }

    #[inline]
    fn write_fixed_litlen(&mut self, sym: u16) {
        match sym {
            0..=143 => self.write_huffman_msb(0x30 + u32::from(sym), 8),
            144..=255 => self.write_huffman_msb(0x190 + u32::from(sym - 144), 9),
            256..=279 => self.write_huffman_msb(u32::from(sym - 256), 7),
            280..=287 => self.write_huffman_msb(0xc0 + u32::from(sym - 280), 8),
            _ => unreachable!("valid RFC 1951 litlen symbol"),
        }
    }

    #[inline]
    fn write_fixed_dist(&mut self, sym: u16) {
        self.write_huffman_msb(u32::from(sym), 5);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bit_count > 0 {
            self.out.push((self.bit_buf & 0xff) as u8);
            self.bit_buf = 0;
            self.bit_count = 0;
        }
        self.out
    }
}

fn deflate_fixed_huffman(input: &[u8]) -> Vec<u8> {
    const WINDOW_SIZE: usize = 32768;
    const HASH_BITS: usize = 14;
    const HASH_SIZE: usize = 1 << HASH_BITS;
    const HASH_MASK: usize = HASH_SIZE - 1;
    const MAX_CHAIN: usize = 32;
    const MAX_MATCH: usize = 258;
    const MIN_MATCH: usize = 3;
    const NIL: usize = usize::MAX;

    let mut head = vec![NIL; HASH_SIZE];
    let mut prev = vec![NIL; WINDOW_SIZE];
    let mut bw = BitWriter::with_capacity(input.len() / 2 + 16);

    // BFINAL=1, BTYPE=01 (fixed Huffman)
    bw.write_bits_lsb(1, 1);
    bw.write_bits_lsb(1, 2);

    let hash3 = |pos: usize| -> usize {
        let a = usize::from(input[pos]);
        let b = usize::from(input[pos + 1]);
        let c = usize::from(input[pos + 2]);
        ((a << 10) ^ (b << 5) ^ c) & HASH_MASK
    };

    let mut pos = 0;
    while pos < input.len() {
        let mut best_len = 0usize;
        let mut best_dist = 0usize;

        if pos + MIN_MATCH <= input.len() {
            let h = hash3(pos);
            let mut cand = head[h];
            prev[pos & (WINDOW_SIZE - 1)] = cand;
            head[h] = pos;

            let max_len = (input.len() - pos).min(MAX_MATCH);
            let mut chain = 0;
            while cand != NIL && chain < MAX_CHAIN {
                let dist = pos - cand;
                if dist == 0 || dist > WINDOW_SIZE {
                    break;
                }
                if input[cand + best_len] == input[pos + best_len]
                    && input[cand] == input[pos]
                    && input[cand + 1] == input[pos + 1]
                    && input[cand + 2] == input[pos + 2]
                {
                    let mut mlen = MIN_MATCH;
                    while mlen < max_len && input[cand + mlen] == input[pos + mlen] {
                        mlen += 1;
                    }
                    if mlen > best_len {
                        best_len = mlen;
                        best_dist = dist;
                        if best_len == max_len {
                            break;
                        }
                    }
                }
                let next = prev[cand & (WINDOW_SIZE - 1)];
                if next >= cand {
                    break;
                }
                cand = next;
                chain += 1;
            }
        }

        if best_len >= MIN_MATCH {
            emit_length_distance(&mut bw, best_len as u16, best_dist as u16);
            // Insert intermediate positions into hash chain
            let end = (pos + best_len).min(input.len());
            let mut p = pos + 1;
            while p < end {
                if p + MIN_MATCH <= input.len() {
                    let h = hash3(p);
                    prev[p & (WINDOW_SIZE - 1)] = head[h];
                    head[h] = p;
                }
                p += 1;
            }
            pos += best_len;
        } else {
            bw.write_fixed_litlen(u16::from(input[pos]));
            pos += 1;
        }
    }

    // End of block (256)
    bw.write_fixed_litlen(256);
    bw.finish()
}

fn emit_length_distance(bw: &mut BitWriter, length: u16, distance: u16) {
    let mut len_idx = 0usize;
    while len_idx + 1 < LENGTH_BASE.len() && LENGTH_BASE[len_idx + 1] <= length {
        len_idx += 1;
    }
    // Special case length == 258 uses symbol 285 (index 28, 0 extra bits)
    if length == 258 {
        len_idx = 28;
    }
    bw.write_fixed_litlen(257 + len_idx as u16);
    let len_extra = LENGTH_EXTRA[len_idx];
    if len_extra > 0 {
        bw.write_bits_lsb(
            u32::from(length - LENGTH_BASE[len_idx]),
            u32::from(len_extra),
        );
    }

    let mut dist_idx = 0usize;
    while dist_idx + 1 < DISTANCE_BASE.len() && DISTANCE_BASE[dist_idx + 1] <= distance {
        dist_idx += 1;
    }
    bw.write_fixed_dist(dist_idx as u16);
    let dist_extra = DISTANCE_EXTRA[dist_idx];
    if dist_extra > 0 {
        bw.write_bits_lsb(
            u32::from(distance - DISTANCE_BASE[dist_idx]),
            u32::from(dist_extra),
        );
    }
}

/// Helper to pack entries into an uncompressed (method 0) ZIP archive.
pub fn build_uncompressed_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut zip = Vec::new();
    let mut cd = Vec::new();

    for (name, content) in entries {
        let offset = zip.len() as u32;
        let fn_bytes = name.as_bytes();
        let fn_len = fn_bytes.len() as u16;
        let size = content.len() as u32;
        let crc = compute_crc32(content);

        // Local header (30 bytes)
        zip.extend_from_slice(&0x04034b50u32.to_le_bytes());
        zip.extend_from_slice(&20u16.to_le_bytes()); // version
        zip.extend_from_slice(&0u16.to_le_bytes()); // flags
        zip.extend_from_slice(&0u16.to_le_bytes()); // method 0 (stored)
        zip.extend_from_slice(&0u16.to_le_bytes()); // time
        zip.extend_from_slice(&0u16.to_le_bytes()); // date
        zip.extend_from_slice(&crc.to_le_bytes());
        zip.extend_from_slice(&size.to_le_bytes()); // compressed size
        zip.extend_from_slice(&size.to_le_bytes()); // uncompressed size
        zip.extend_from_slice(&fn_len.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes()); // extra len
        zip.extend_from_slice(fn_bytes);
        zip.extend_from_slice(content);

        // Central directory entry (46 bytes)
        cd.extend_from_slice(&0x02014b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes()); // version made by
        cd.extend_from_slice(&20u16.to_le_bytes()); // version needed
        cd.extend_from_slice(&0u16.to_le_bytes()); // flags
        cd.extend_from_slice(&0u16.to_le_bytes()); // method 0
        cd.extend_from_slice(&0u16.to_le_bytes()); // time
        cd.extend_from_slice(&0u16.to_le_bytes()); // date
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&size.to_le_bytes());
        cd.extend_from_slice(&size.to_le_bytes());
        cd.extend_from_slice(&fn_len.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes()); // extra len
        cd.extend_from_slice(&0u16.to_le_bytes()); // comment len
        cd.extend_from_slice(&0u16.to_le_bytes()); // disk start
        cd.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        cd.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        cd.extend_from_slice(&offset.to_le_bytes());
        cd.extend_from_slice(fn_bytes);
    }

    let cd_offset = zip.len() as u32;
    let cd_size = cd.len() as u32;
    let num_entries = entries.len() as u16;

    zip.extend_from_slice(&cd);

    // EOCD (22 bytes)
    zip.extend_from_slice(&0x06054b50u32.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes()); // disk num
    zip.extend_from_slice(&0u16.to_le_bytes()); // cd start disk
    zip.extend_from_slice(&num_entries.to_le_bytes());
    zip.extend_from_slice(&num_entries.to_le_bytes());
    zip.extend_from_slice(&cd_size.to_le_bytes());
    zip.extend_from_slice(&cd_offset.to_le_bytes());
    zip.extend_from_slice(&0u16.to_le_bytes()); // comment len

    zip
}

fn compute_crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffffffffu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1) != 0;
            crc >>= 1;
            if mask {
                crc ^= 0xedb88320;
            }
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zip_roundtrip_stored() {
        let files = [
            ("catalog/Swallow.pkl", b"swallow = 1\n".as_slice()),
            ("Bird.pkl", b"bird = 2\n".as_slice()),
        ];
        let zip_bytes = build_uncompressed_zip(&files);
        let archive = ZipArchive::parse(&zip_bytes).unwrap();
        assert_eq!(archive.get("/Bird.pkl").unwrap(), b"bird = 2\n");
        assert_eq!(
            archive.get("/catalog/Swallow.pkl").unwrap(),
            b"swallow = 1\n"
        );
        assert!(archive.has_element("/catalog"));
        assert!(archive.has_element("/catalog/Swallow.pkl"));
        let elements = archive.list_elements("/catalog");
        assert_eq!(elements, vec![("Swallow.pkl".into(), false)]);
    }

    #[test]
    fn test_zip_directory_reading_and_module_load_distinction() {
        let files = [
            ("catalog/Swallow.pkl", b"swallow = 1\n".as_slice()),
            ("catalog/Ostrich.pkl", b"ostrich = 2\n".as_slice()),
            ("Bird.pkl", b"bird = 3\n".as_slice()),
        ];
        let zip_bytes = build_uncompressed_zip(&files);
        let archive = ZipArchive::parse(&zip_bytes).unwrap();

        // Reading directory with allow_directories = true returns sorted child names joined by \n with trailing \n
        let dir_bytes = archive.read_bytes("/catalog", true).unwrap();
        assert_eq!(
            String::from_utf8(dir_bytes).unwrap(),
            "Ostrich.pkl\nSwallow.pkl\n"
        );

        let root_bytes = archive.read_bytes("/", true).unwrap();
        assert_eq!(
            String::from_utf8(root_bytes).unwrap(),
            "Bird.pkl\ncatalog\n"
        );

        // Reading directory with allow_directories = false fails with IsADirectory
        assert_eq!(
            archive.read_bytes("/catalog", false),
            Err(ZipReadError::IsADirectory)
        );
        assert_eq!(
            archive.read_bytes("/", false),
            Err(ZipReadError::IsADirectory)
        );

        // Missing entry returns NotFound
        assert_eq!(
            archive.read_bytes("/missing.pkl", false),
            Err(ZipReadError::NotFound)
        );
    }

    #[test]
    fn test_normalize_zip_path_clamped_at_root() {
        assert_eq!(normalize_zip_path(""), "/");
        assert_eq!(normalize_zip_path("/"), "/");
        assert_eq!(normalize_zip_path("/.."), "/");
        assert_eq!(normalize_zip_path("/../../.."), "/");
        assert_eq!(normalize_zip_path("/foo/../../penguin.pkl"), "/penguin.pkl");
        assert_eq!(
            normalize_zip_path("catalog/Swallow.pkl"),
            "/catalog/Swallow.pkl"
        );
        assert_eq!(
            normalize_zip_path("/catalog/../catalog/Ostrich.pkl"),
            "/catalog/Ostrich.pkl"
        );
    }

    #[test]
    fn test_bit3_streaming_entry_reads_compressed_size_from_central_directory() {
        let mut zip = Vec::new();
        let content = b"streaming content here";
        let crc = compute_crc32(content);
        let fn_bytes = b"stream.txt";
        let fn_len = fn_bytes.len() as u16;
        let size = content.len() as u32;

        let local_offset = zip.len() as u32;
        // Local header with local_compressed_size = 0 and local_uncompressed_size = 0 (Bit 3 flag)
        zip.extend_from_slice(&0x04034b50u32.to_le_bytes());
        zip.extend_from_slice(&20u16.to_le_bytes()); // version
        zip.extend_from_slice(&0x0008u16.to_le_bytes()); // flags: bit 3 set
        zip.extend_from_slice(&0u16.to_le_bytes()); // method 0 (stored)
        zip.extend_from_slice(&0u16.to_le_bytes()); // time
        zip.extend_from_slice(&0u16.to_le_bytes()); // date
        zip.extend_from_slice(&0u32.to_le_bytes()); // zero crc
        zip.extend_from_slice(&0u32.to_le_bytes()); // zero compressed size in local header
        zip.extend_from_slice(&0u32.to_le_bytes()); // zero uncompressed size in local header
        zip.extend_from_slice(&fn_len.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(fn_bytes);
        zip.extend_from_slice(content);

        // Central directory entry has the real sizes
        let cd_offset = zip.len() as u32;
        let mut cd = Vec::new();
        cd.extend_from_slice(&0x02014b50u32.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&20u16.to_le_bytes());
        cd.extend_from_slice(&0x0008u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&crc.to_le_bytes());
        cd.extend_from_slice(&size.to_le_bytes()); // compressed size in CD
        cd.extend_from_slice(&size.to_le_bytes()); // uncompressed size in CD
        cd.extend_from_slice(&fn_len.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u16.to_le_bytes());
        cd.extend_from_slice(&0u32.to_le_bytes());
        cd.extend_from_slice(&local_offset.to_le_bytes());
        cd.extend_from_slice(fn_bytes);

        let cd_size = cd.len() as u32;
        zip.extend_from_slice(&cd);

        // EOCD
        zip.extend_from_slice(&0x06054b50u32.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());
        zip.extend_from_slice(&1u16.to_le_bytes());
        zip.extend_from_slice(&1u16.to_le_bytes());
        zip.extend_from_slice(&cd_size.to_le_bytes());
        zip.extend_from_slice(&cd_offset.to_le_bytes());
        zip.extend_from_slice(&0u16.to_le_bytes());

        let archive = ZipArchive::parse(&zip).unwrap();
        assert_eq!(archive.get("/stream.txt").unwrap(), content);
    }

    #[test]
    fn test_trailing_bits_less_than_9_at_eof() {
        // Huffman table with 3-bit code
        let lengths = [3u8, 3, 3, 3, 3, 3, 3, 3];
        let huffman = FastHuffman::from_code_lengths(&lengths).unwrap();
        // A single byte containing 3 bits of symbol 0 followed by 5 unused bits
        let data = [0b00000000u8];
        let mut reader = BitReader::new(&data);
        let sym = huffman.decode(&mut reader).unwrap();
        assert_eq!(sym, 0);
        assert_eq!(reader.bit_pos, 3);
    }

    #[test]
    fn zip_parser_rejects_traversal_oversized_header_and_skips_symlinks() {
        // 1. Entry with `..` traversal is rejected.
        let bad = build_deterministic_package_zip(&[("safe/../escape.pkl", b"a = 1")]);
        assert!(ZipArchive::parse(&bad).is_err());

        // 2. Entry claiming 200 MiB uncompressed in header is rejected before allocating.
        let mut bomb = build_deterministic_package_zip(&[("a.txt", b"hi")]);
        // Patch CD uncompressed_size to 200 MiB (0x0c80_0000)
        let cd_sig = [0x50, 0x4b, 0x01, 0x02];
        let cd_pos = bomb.windows(4).position(|w| w == cd_sig).unwrap();
        bomb[cd_pos + 24..cd_pos + 28].copy_from_slice(&(200u32 * 1024 * 1024).to_le_bytes());
        assert!(ZipArchive::parse(&bomb).is_err());

        // 3. Deeply nested path (2000 segments) succeeds iteratively without stack overflow.
        let deep_name = format!("{}leaf.pkl", "d/".repeat(2000));
        let deep = build_deterministic_package_zip(&[(&deep_name, b"x = 1")]);
        let parsed = ZipArchive::parse(&deep).unwrap();
        assert_eq!(parsed.get(&format!("/{deep_name}")).unwrap(), b"x = 1");

        // 4. Unix symlink entry (`S_IFLNK = 0o120000` in external attributes) is skipped.
        let mut sym = build_deterministic_package_zip(&[("link.pkl", b"/etc/passwd")]);
        let cd_pos = sym.windows(4).position(|w| w == cd_sig).unwrap();
        // version_made_by = Unix (3 << 8)
        sym[cd_pos + 4..cd_pos + 6].copy_from_slice(&((3u16 << 8) | 20).to_le_bytes());
        // external_attr = 0o120777 << 16
        sym[cd_pos + 38..cd_pos + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
        let parsed_sym = ZipArchive::parse(&sym).unwrap();
        assert!(parsed_sym.get("/link.pkl").is_none());

        // 5. macOS symlink entry (version_made_by OS == 19) is also skipped.
        let mut mac_sym = build_deterministic_package_zip(&[("maclink.pkl", b"/etc/passwd")]);
        let cd_pos = mac_sym.windows(4).position(|w| w == cd_sig).unwrap();
        mac_sym[cd_pos + 4..cd_pos + 6].copy_from_slice(&((19u16 << 8) | 20).to_le_bytes());
        mac_sym[cd_pos + 38..cd_pos + 42].copy_from_slice(&(0o120777u32 << 16).to_le_bytes());
        let parsed_mac_sym = ZipArchive::parse(&mac_sym).unwrap();
        assert!(parsed_mac_sym.get("/maclink.pkl").is_none());

        // 6. Path exceeding MAX_ZIP_PATH_BYTES or MAX_ZIP_PATH_DEPTH is rejected cleanly.
        let too_deep = format!("{}leaf.pkl", "d/".repeat(MAX_ZIP_PATH_DEPTH + 10));
        let deep_zip = build_deterministic_package_zip(&[(&too_deep, b"")]);
        assert!(ZipArchive::parse(&deep_zip).is_err());

        // 7. Zero-payload directory-hierarchy bomb exceeding MAX_ZIP_DIR_METADATA_BYTES is rejected.
        let mut many_names = Vec::new();
        for i in 0..160 {
            let mut path = format!("p{i:03}/");
            for j in 0..240 {
                path.push_str(&format!("d{j:03}/"));
            }
            path.push_str("f.pkl");
            many_names.push(path);
        }
        let entries: Vec<(&str, &[u8])> =
            many_names.iter().map(|s| (s.as_str(), &b""[..])).collect();
        let bomb_zip = build_uncompressed_zip(&entries);
        assert!(ZipArchive::parse(&bomb_zip).is_err());
    }
}
