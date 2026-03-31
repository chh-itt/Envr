use envr_error::{EnvrError, EnvrResult};
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

pub fn sha256_hex(path: impl AsRef<Path>) -> EnvrResult<String> {
    let mut f = File::open(path.as_ref()).map_err(EnvrError::from)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf).map_err(EnvrError::from)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn verify_sha256_hex(path: impl AsRef<Path>, expected_hex: &str) -> EnvrResult<()> {
    let got = sha256_hex(path)?;
    if !eq_hex_case_insensitive(&got, expected_hex) {
        return Err(EnvrError::Validation(format!(
            "sha256 mismatch: expected {expected_hex}, got {got}"
        )));
    }
    Ok(())
}

fn eq_hex_case_insensitive(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .all(|(x, y)| x.eq_ignore_ascii_case(&y))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn sha256_is_stable() {
        let tmp = TempDir::new().expect("tmp");
        let p = tmp.path().join("x.bin");
        fs::write(&p, b"abc").expect("write");

        let got = sha256_hex(&p).expect("sha");
        // known sha256("abc")
        assert_eq!(
            got,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
