//! Embeds the built-in skill manifests: lists `skills/<id>/manifest.toml` at
//! the workspace root and writes an `include_str!` table for each one, so
//! the binary needs no file access at run time to know its skills.

use std::error::Error;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::{env, fs};

fn main() -> Result<(), Box<dyn Error>> {
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let skills_dir = crate_dir.join("../../skills").canonicalize()?;
    println!("cargo::rerun-if-changed={}", skills_dir.display());

    let mut manifests = Vec::new();
    for entry in fs::read_dir(&skills_dir)? {
        let dir = entry?.path();
        let manifest = dir.join("manifest.toml");
        if manifest.is_file() {
            let id = dir
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("skill directory name is not UTF-8")?
                .to_owned();
            println!("cargo::rerun-if-changed={}", manifest.display());
            manifests.push((id, manifest));
        }
    }
    // Sorted, so the generated table does not depend on directory order.
    manifests.sort();

    let mut code =
        String::from("/// Built-in manifests as (directory, TOML text), by directory name.\n");
    code.push_str("pub(crate) const MANIFESTS: &[(&str, &str)] = &[\n");
    for (id, path) in &manifests {
        let path = path.to_str().ok_or("manifest path is not UTF-8")?;
        writeln!(code, "    ({id:?}, include_str!({path:?})),")?;
    }
    code.push_str("];\n");

    let out = PathBuf::from(env::var("OUT_DIR")?).join("manifests.rs");
    fs::write(out, code)?;
    Ok(())
}
