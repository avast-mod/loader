use color_eyre::eyre::OptionExt;
use include_dir::{Dir, DirEntry, include_dir};
use std::collections::HashMap;

pub const BUILTIN_MOD_ID: &str = "avast";

static BUILTIN_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/mods/builtin");

fn extract(result: &mut HashMap<String, Vec<u8>>, dir: &Dir) -> color_eyre::Result<()> {
    for entry in dir.entries() {
        let path = entry
            .path()
            .to_str()
            .ok_or_eyre("failed to stringify path")?;

        match entry {
            DirEntry::Dir(d) => extract(result, d)?,
            DirEntry::File(f) => {
                result.insert(path.to_string(), f.contents().to_vec());
            }
        }
    }

    Ok(())
}

pub fn create_builtin_mod() -> color_eyre::Result<HashMap<String, Vec<u8>>> {
    let mut result = HashMap::new();
    extract(&mut result, &BUILTIN_DIR)?;

    let mod_info = format!(
        "[mod]\nid = \"{BUILTIN_MOD_ID}\"\nname = \"AVaSt\"\nversion = \"{}\"\n",
        env!("CARGO_PKG_VERSION")
    );
    result.insert("mod.cfg".to_string(), mod_info.into_bytes());

    Ok(result)
}
