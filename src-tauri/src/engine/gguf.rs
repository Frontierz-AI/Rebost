//! Read just enough of a GGUF header to learn the trained context length.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const GGUF_MAGIC: &[u8; 4] = b"GGUF";
const MAX_HEADER_BYTES: u64 = 2 * 1024 * 1024;
const MAX_KV: u64 = 512;
const MAX_KEY: u64 = 256;

const TY_UINT8: u32 = 0;
const TY_INT8: u32 = 1;
const TY_UINT16: u32 = 2;
const TY_INT16: u32 = 3;
const TY_UINT32: u32 = 4;
const TY_INT32: u32 = 5;
const TY_FLOAT32: u32 = 6;
const TY_BOOL: u32 = 7;
const TY_STRING: u32 = 8;
const TY_ARRAY: u32 = 9;
const TY_UINT64: u32 = 10;
const TY_INT64: u32 = 11;
const TY_FLOAT64: u32 = 12;

/// Trained context (`*.context_length`) when the header is readable.
pub fn read_context_length(path: &Path) -> Option<u32> {
    let mut file = File::open(path).ok()?;
    let mut magic = [0u8; 4];
    file.read_exact(&mut magic).ok()?;
    if &magic != GGUF_MAGIC {
        return None;
    }
    let version = read_u32(&mut file)?;
    if !(2..=3).contains(&version) {
        return None;
    }
    let _n_tensors = read_u64(&mut file)?;
    let n_kv = read_u64(&mut file)?;
    if n_kv == 0 || n_kv > MAX_KV {
        return None;
    }
    for _ in 0..n_kv {
        if file.stream_position().ok()? > MAX_HEADER_BYTES {
            return None;
        }
        let key = read_string(&mut file)?;
        let ty = read_u32(&mut file)?;
        if key.ends_with(".context_length") || key == "context_length" {
            return read_int_value(&mut file, ty);
        }
        skip_value(&mut file, ty)?;
    }
    None
}

fn read_int_value(file: &mut File, ty: u32) -> Option<u32> {
    match ty {
        TY_UINT32 | TY_INT32 => read_u32(file),
        TY_UINT16 | TY_INT16 => {
            let mut buf = [0u8; 2];
            file.read_exact(&mut buf).ok()?;
            Some(u16::from_le_bytes(buf) as u32)
        }
        TY_UINT64 | TY_INT64 => {
            let n = read_u64(file)?;
            u32::try_from(n).ok()
        }
        _ => None,
    }
}

fn skip_value(file: &mut File, ty: u32) -> Option<()> {
    let skip = match ty {
        TY_UINT8 | TY_INT8 | TY_BOOL => 1,
        TY_UINT16 | TY_INT16 => 2,
        TY_UINT32 | TY_INT32 | TY_FLOAT32 => 4,
        TY_UINT64 | TY_INT64 | TY_FLOAT64 => 8,
        TY_STRING => {
            let n = read_u64(file)?;
            if n > MAX_HEADER_BYTES {
                return None;
            }
            file.seek(SeekFrom::Current(n as i64)).ok()?;
            return Some(());
        }
        TY_ARRAY => {
            let elem = read_u32(file)?;
            let count = read_u64(file)?;
            if count > 10_000 {
                return None;
            }
            for _ in 0..count {
                skip_value(file, elem)?;
            }
            return Some(());
        }
        _ => return None,
    };
    file.seek(SeekFrom::Current(skip)).ok()?;
    Some(())
}

fn read_string(file: &mut File) -> Option<String> {
    let n = read_u64(file)?;
    if n > MAX_KEY {
        return None;
    }
    let mut buf = vec![0u8; n as usize];
    file.read_exact(&mut buf).ok()?;
    String::from_utf8(buf).ok()
}

fn read_u32(file: &mut File) -> Option<u32> {
    let mut buf = [0u8; 4];
    file.read_exact(&mut buf).ok()?;
    Some(u32::from_le_bytes(buf))
}

fn read_u64(file: &mut File) -> Option<u64> {
    let mut buf = [0u8; 8];
    file.read_exact(&mut buf).ok()?;
    Some(u64::from_le_bytes(buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gguf_with_ctx(n: u32) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(GGUF_MAGIC);
        bytes.extend_from_slice(&3u32.to_le_bytes());
        bytes.extend_from_slice(&0u64.to_le_bytes());
        bytes.extend_from_slice(&1u64.to_le_bytes());
        let key = b"llama.context_length";
        bytes.extend_from_slice(&(key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(key);
        bytes.extend_from_slice(&TY_UINT32.to_le_bytes());
        bytes.extend_from_slice(&n.to_le_bytes());
        bytes
    }

    #[test]
    fn reads_trained_context_from_a_header() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.gguf");
        std::fs::write(&path, gguf_with_ctx(32_768)).unwrap();
        assert_eq!(read_context_length(&path), Some(32_768));
    }

    #[test]
    fn rejects_a_non_gguf_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("m.bin");
        std::fs::write(&path, b"not a gguf").unwrap();
        assert_eq!(read_context_length(&path), None);
    }
}
