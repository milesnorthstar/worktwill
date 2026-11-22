use base64::{engine::general_purpose::STANDARD, Engine};
use minisign_verify::{PublicKey, Signature};
use std::{error::Error, fs, path::Path};

fn verify(
    data: &[u8],
    signature: &str,
    public_key: &str,
    version: &str,
) -> Result<(), Box<dyn Error>> {
    let key = String::from_utf8(STANDARD.decode(public_key.trim())?)?;
    let signed = String::from_utf8(STANDARD.decode(signature.trim())?)?;
    let key = PublicKey::decode(&key)?;
    let signed = Signature::decode(&signed)?;
    // 全局签名验证成功后才能信任版本注释；与客户端使用同一验签库。
    key.verify(data, &signed, true)?;
    let versions: Vec<_> = signed
        .trusted_comment()
        .split('\t')
        .filter_map(|field| field.strip_prefix("version:"))
        .collect();
    if versions != [version] {
        return Err("signature must bind exactly the announced version".into());
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("usage: verify-release <package> <signature> <tauri-config> <version>".into());
    }
    if fs::metadata(&args[0])?.len() > 512 * 1024 * 1024 {
        return Err("update package exceeds client size limit".into());
    }
    let config: serde_json::Value = serde_json::from_slice(&fs::read(Path::new(&args[2]))?)?;
    let public_key = config["plugins"]["updater"]["pubkey"]
        .as_str()
        .ok_or("missing updater public key")?;
    verify(
        &fs::read(&args[0])?,
        &fs::read_to_string(&args[1])?,
        public_key,
        &args[3],
    )?;
    println!("Update signature and version verified.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const DATA: &[u8] = include_bytes!("../../tests/fixtures/updates/payload");
    const SIGNATURE: &str = include_str!("../../tests/fixtures/updates/payload.sig");
    const KEY: &str = include_str!("../../tests/fixtures/updates/public.key");

    #[test]
    fn verifies_exact_bytes_key_and_signed_version() {
        assert!(verify(DATA, SIGNATURE, KEY, "2.0.0").is_ok());
        assert!(verify(b"changed", SIGNATURE, KEY, "2.0.0").is_err());
        assert!(verify(DATA, SIGNATURE, KEY, "3.0.0").is_err());
        assert!(verify(DATA, SIGNATURE, "invalid", "2.0.0").is_err());
    }
}
