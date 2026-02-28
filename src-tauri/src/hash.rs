use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};

pub fn sha256_file_hex(path: &Path) -> Result<String, String> {
    let mut f = fs::File::open(path).map_err(|e| format!("open file for sha256: {e}"))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 64];
    loop {
        let n = f
            .read(&mut buf)
            .map_err(|e| format!("read file for sha256: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}
