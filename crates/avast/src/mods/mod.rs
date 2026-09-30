use crate::mods::builtin::create_builtin_mod;
use crate::mods::config::ModConfig;
use crate::mods::filesystem::{ModLoaderFolderFs, ModLoaderFs, ModLoaderMapFs};
use crate::virtual_pack::FileContents;
use color_eyre::eyre::{Context, Report, bail};
use avast_godot::build::GDScriptV2Build;
use avast_godot::gdscript::{Spanned, Token};
use avast_godot::pack::{Pack, PackConfig};
use avast_godot::project_settings::ProjectSettings;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

mod builtin;
mod config;
mod filesystem;

pub use builtin::BUILTIN_MOD_ID;

/// Optional metadata for the mod.
/// These fields are not used by AVaSt directly, but may be used by other mods (e.g. config UIs).
/// If you publish your mod online, we suggest making sure these values are in sync with your mod page.
#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ModMeta {
    /// Pretty name for this mod.
    pub name: Option<String>,

    /// Version number. No strict format is imposed on this, but it should be obvious to users.
    pub version: Option<String>,

    /// A list of mod authors.
    #[serde(default)]
    pub authors: Vec<String>,

    /// A short description of what this mod does.
    pub description: Option<String>,

    /// A link to a website (such as mod page or source code) for this mod.
    pub website: Option<String>,
}

/// Optional metadata for a mod's config section.
/// These fields are not used by AVaSt directly, but may be used by other mods (e.g. config UIs).
#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ModConfigOptionMeta {
    /// Pretty name for this option.
    pub name: Option<String>,

    /// Description for this option.
    pub description: Option<String>,

    /// Type for this option, to be used as a hint for custom config editors.
    /// AVaSt does not perform any type checking, and the value saved in this option may not match the type.
    pub r#type: Option<ModConfigOptionType>,

    /// The default value for this option.
    /// The default value will not be written to the config file directly, but will be returned by the config APIs and displayed in comments.
    pub default: Option<toml::Value>,

    /// Whether to hide all comments for this option.
    #[serde(default)]
    pub hidden: bool,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum ModConfigOptionType {
    String,
    Number,
    Boolean,
    Array,
    Table,
}

#[derive(Deserialize, Serialize, Debug, Clone)]
pub struct ModInfo {
    /// A unique ID for this mod. Mod IDs should use snake case (all lowercase, with spaces replaced with underscores).
    /// This is the only required field in the mod info.
    pub id: String,

    /// Human-readable metadata about this mod.
    pub meta: Option<ModMeta>,

    /// Optional metadata for the mod's config options. Config options are referenced by a section ID and option ID.
    /// The "meta" option ID is reserved to represent metadata about the section itself.
    pub config: Option<ModConfigInfo>,

    /// Game builds this mod supports. Absent bounds mean no limit on that side.
    pub compat: Option<ModCompat>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct ModCompat {
    pub game_min: Option<u64>,
    pub game_max: Option<u64>,
}

pub const SUPPORTED_GAME_BUILD: u64 = 25578107;

pub type ModConfigInfo = IndexMap<String, IndexMap<String, ModConfigOptionMeta>>;

#[derive(Debug, Default)]
pub struct PatcherCallbacks;

impl PatcherCallbacks {
    pub fn patch_script(
        &self,
        _path: &str,
        tokens: Vec<Spanned<Token>>,
        _version: &GDScriptV2Build,
    ) -> color_eyre::Result<Vec<Spanned<Token>>> {
        Ok(tokens)
    }

    pub fn has_patcher_for_script(&self, _path: &str) -> bool {
        false
    }

    pub fn patch_project_settings(
        &self,
        settings: ProjectSettings,
    ) -> color_eyre::Result<ProjectSettings> {
        Ok(settings)
    }

    pub fn has_patcher_for_file(&self, _path: &str) -> bool {
        false
    }

    pub fn patch_file(&self, _path: &str, input: &[u8]) -> color_eyre::Result<Vec<u8>> {
        Ok(input.to_vec())
    }
}

impl ModInfo {
    /// Parse the config.
    pub fn parse(data: &str) -> color_eyre::Result<Self> {
        let mut section = String::new();
        let mut values: HashMap<(String, String), String> = HashMap::new();
        for raw in data.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }
            if let Some(name) = line
                .strip_prefix('[')
                .and_then(|s| s.strip_suffix(']'))
            {
                section = name.trim().to_string();
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                bail!("Invalid line in mod.cfg: {raw}");
            };
            let value = value.trim();
            let value = value
                .strip_prefix('"')
                .and_then(|s| s.strip_suffix('"'))
                .unwrap_or(value);
            values.insert((section.clone(), key.trim().to_string()), value.to_string());
        }
        let get = |s: &str, k: &str| values.get(&(s.to_string(), k.to_string())).cloned();
        let Some(id) = get("mod", "id") else {
            bail!("mod.cfg is missing [mod] id");
        };
        let meta = ModMeta {
            name: get("mod", "name"),
            version: get("mod", "version"),
            authors: get("mod", "authors")
                .map(|a| {
                    a.split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default(),
            description: get("mod", "description"),
            website: get("mod", "website"),
        };
        let meta = if meta.name.is_none()
            && meta.version.is_none()
            && meta.authors.is_empty()
            && meta.description.is_none()
            && meta.website.is_none()
        {
            None
        } else {
            Some(meta)
        };
        let compat = ModCompat {
            game_min: get("compat", "game_min").and_then(|s| s.parse().ok()),
            game_max: get("compat", "game_max").and_then(|s| s.parse().ok()),
        };
        let compat = if compat.game_min.is_none() && compat.game_max.is_none() {
            None
        } else {
            Some(compat)
        };
        Ok(Self {
            id,
            meta,
            config: None,
            compat,
        })
    }
}

#[derive(Debug)]
pub enum ModPack {
    /// The mod uses a direct .pck file.
    PackFile { pack: Pack, mapping: FileContents },

    /// The mod uses a loose directory.
    LooseDir {
        files: HashMap<String, FileContents>,
    },
}

impl ModPack {
    // FIXME this shit is ass and would be better served by an iter but I'm lazy
    pub fn files(&self) -> HashMap<String, FileContents> {
        let mut result = HashMap::new();

        match self {
            ModPack::PackFile { pack, mapping } => {
                let mapping = match mapping {
                    FileContents::Disk { mapping, .. } => mapping,
                    _ => unimplemented!(),
                };

                for (path, file) in &pack.files {
                    let contents = FileContents::Disk {
                        mapping: mapping.clone(),
                        offset: file.offset,
                        len: file.size,
                    };

                    result.insert(path.clone(), contents);
                }
            }

            ModPack::LooseDir { files } => {
                for (path, contents) in files {
                    result.insert(path.clone(), contents.clone());
                }
            }
        }

        result
    }
}

    /// A loaded mod.
#[derive(Debug)]
pub struct Mod {
    /// The directory where the mod is stored.
    pub root_directory: Option<PathBuf>,

    /// Static info on the mod (ID, metadata, etc.).
    pub info: ModInfo,

    /// The mod's pack. This will be empty if there is no `data.pck` in the mod folder.
    pub pack: Option<ModPack>,

    /// The mod's config.
    pub config: ModConfig,
}

#[derive(Debug)]
pub struct Mods(pub HashMap<String, Mod>);

impl Mods {
    /// Reads a mod from a directory. Errors if there is issues with the mod (e.g. invalid or
    /// missing mod info).
    fn read_mod_from_directory(
        fs: &dyn ModLoaderFs,
        configs_directory: &Path,
        pack_config: PackConfig,
    ) -> color_eyre::Result<Mod> {
        // Check for a `mod.cfg` file.
        let mod_info_path = PathBuf::from("mod.cfg");
        if !fs.exists(&mod_info_path)? {
            bail!("Mod is missing a mod.cfg");
        }

        let mod_info = fs.read(&mod_info_path).wrap_err("reading mod.cfg")?;
        let mod_info = mod_info.as_slice();
        let mod_info = std::str::from_utf8(mod_info).wrap_err("parsing mod.cfg")?;
        let mod_info = ModInfo::parse(mod_info)?;

        if mod_info
            .id
            .chars()
            .any(|c| !c.is_alphanumeric() && c != '_')
        {
            // mainly to avoid any filesystem shenanigans
            bail!("Mod contains improperly formatted mod ID");
        }

        // Search for mod data.
        let pck_path = PathBuf::from("data.pck");
        let pack_folder = PathBuf::from("data");
        let pack = if fs.exists(&pck_path)? {
            let file = fs.read(&pck_path).wrap_err("reading data.pck")?;
            let cursor = Cursor::new(file.as_slice());
            let pack = Pack::parse(cursor, pack_config).wrap_err("reading pck")?;

            Some(ModPack::PackFile {
                pack,
                mapping: file,
            })
        } else {
            if fs.exists(&pack_folder)? && fs.is_dir(&pack_folder)? {
                let mut files = HashMap::<String, FileContents>::new();
                read_files_recursively(fs, &mut files, &pack_folder, None)?;
                Some(ModPack::LooseDir { files })
            } else {
                None
            }
        };

        let config_path = configs_directory.join(format!("{}.toml", mod_info.id));
        let config = ModConfig::new(config_path, mod_info.config.clone().unwrap_or_default());

        Ok(Mod {
            root_directory: fs.root(),
            info: mod_info,
            pack,
            config,
        })
    }

    fn read_mod_from_avast_file(
        path: &Path,
        configs_directory: &Path,
        pack_config: PackConfig,
    ) -> color_eyre::Result<Mod> {
        let file = std::fs::File::open(path).wrap_err("reading .avast file")?;
        let mut archive = zip::ZipArchive::new(file).wrap_err("parsing .avast file")?;

        let mut files = HashMap::<String, Vec<u8>>::new();
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).wrap_err("reading .avast entry")?;
            if !entry.is_file() {
                continue;
            }
            let name = entry.name().to_string();
            let mut contents = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut contents)
                .wrap_err("reading .avast entry")?;
            files.insert(name, contents);
        }

        let fs = ModLoaderMapFs::new(files);
        let mut r#mod = Mods::read_mod_from_directory(&fs, configs_directory, pack_config)?;
        r#mod.root_directory = Some(path.to_path_buf());
        Ok(r#mod)
    }

    /// Searches for mod folders in the given directory and loads their metadata/patchers/etc.
    pub fn search_and_load(
        mods_directory: &Path,
        configs_directory: &Path,
        pack_config: PackConfig,
    ) -> Result<Self, Vec<Report>> {
        let mut errors = Vec::new();

        if let Err(err) = std::fs::create_dir_all(mods_directory) {
            errors.push(Report::from(err).wrap_err("attempting to create mods folder"));
            return Err(errors);
        }

        if let Err(err) = std::fs::create_dir_all(configs_directory) {
            errors.push(Report::from(err).wrap_err("attempting to create configs folder"));
            return Err(errors);
        }

        let candidate_iter = match std::fs::read_dir(mods_directory) {
            Ok(it) => it,
            Err(err) => {
                errors.push(Report::from(err).wrap_err("listing mods folder"));
                return Err(errors);
            }
        };

        let mut mods = HashMap::new();

        // Load built-in mod.
        {
            let fs = match create_builtin_mod() {
                Ok(fs) => fs,
                Err(err) => {
                    errors.push(err.wrap_err("loading builtin mod"));
                    return Err(errors);
                }
            };
            let fs = ModLoaderMapFs::new(fs);

            match Mods::read_mod_from_directory(&fs, configs_directory, pack_config.clone()) {
                Ok(r#mod) => {
                    mods.insert(r#mod.info.id.clone(), r#mod);
                }

                Err(err) => {
                    tracing::error!(?err, "failed to load builtin mod");
                }
            }
        }

        for candidate in candidate_iter {
            let candidate = match candidate {
                Ok(v) => v,
                Err(err) => {
                    let msg = "reading mod candidate directory";
                    errors.push(Report::from(err).wrap_err(msg));
                    continue;
                }
            };

            let candidate_path = candidate.path();

            let is_avast_bundle = candidate_path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("avast"));
            if !candidate_path.is_dir() && !is_avast_bundle {
                continue;
            }

            let relative_path = candidate_path
                .strip_prefix(mods_directory)
                .expect("directory doesn't have parent path as prefix?");

            let loaded = if is_avast_bundle {
                Mods::read_mod_from_avast_file(
                    &candidate_path,
                    configs_directory,
                    pack_config.clone(),
                )
            } else {
                let fs = ModLoaderFolderFs::new(candidate_path.clone());
                Mods::read_mod_from_directory(&fs, configs_directory, pack_config.clone())
            };
            match loaded {
                Ok(r#mod) => {
                    if let Err(err) = check_compat(&r#mod.info) {
                        tracing::error!(mod_id = r#mod.info.id, ?err, "skipping incompatible mod");
                        continue;
                    }
                    if mods.contains_key(&r#mod.info.id) {
                        tracing::warn!(
                            mod_id = r#mod.info.id,
                            "attempted to load duplicate mod ID"
                        );
                        continue;
                    }

                    mods.insert(r#mod.info.id.clone(), r#mod);
                }
                Err(err) => {
                    let relative_path = relative_path.display();
                    tracing::error!(?err, %relative_path, "failed to load mod");
                }
            }
        }

        Ok(Mods(mods))
    }
}

fn check_compat(info: &ModInfo) -> color_eyre::Result<()> {
    let Some(compat) = &info.compat else {
        return Ok(());
    };
    if let Some(min) = compat.game_min
        && SUPPORTED_GAME_BUILD < min
    {
        bail!(
            "mod needs game build {min} or newer, loader supports {SUPPORTED_GAME_BUILD}"
        );
    }
    if let Some(max) = compat.game_max
        && SUPPORTED_GAME_BUILD > max
    {
        bail!("mod supports up to game build {max}, loader supports {SUPPORTED_GAME_BUILD}");
    }
    Ok(())
}

fn read_files_recursively(
    fs: &dyn ModLoaderFs,
    result: &mut HashMap<String, FileContents>,
    dir: &Path,
    prefix: Option<String>,
) -> color_eyre::Result<()> {
    let files = fs.read_dir(dir)?;

    for file in files {
        let file_path = dir.join(&file);

        let file_name = prefix
            .as_ref()
            .map(|old| format!("{}/{}", old, file))
            .unwrap_or_else(|| file);

        if fs.is_dir(&file_path)? {
            read_files_recursively(fs, result, &file_path, Some(file_name))?;
        } else {
            let contents = fs.read(&file_path)?;
            result.insert(file_name, contents);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_api_style_mod_cfg() {
        let info = ModInfo::parse(
            "[mod]\nid=\"showcase\"\nversion=\"1.0.0\"\n\n[compat]\ngame_min=\"25578107\"\n",
        )
        .unwrap();
        assert_eq!(info.id, "showcase");
        let meta = info.meta.unwrap();
        assert_eq!(meta.version.as_deref(), Some("1.0.0"));
        assert!(info.config.is_none());
    }

    #[test]
    fn rejects_missing_id() {
        assert!(ModInfo::parse("[mod]\nversion=\"1.0.0\"\n").is_err());
    }

    #[test]
    fn compat_range() {
        let info = ModInfo::parse(
            "[mod]\nid=\"showcase\"\n\n[compat]\ngame_min=\"25578107\"\ngame_max=\"25578107\"\n",
        )
        .unwrap();
        assert!(check_compat(&info).is_ok());
        let info = ModInfo::parse("[mod]\nid=\"showcase\"\n\n[compat]\ngame_min=\"99999999\"\n").unwrap();
        assert!(check_compat(&info).is_err());
        let info = ModInfo::parse("[mod]\nid=\"showcase\"\n\n[compat]\ngame_max=\"1\"\n").unwrap();
        assert!(check_compat(&info).is_err());
        let info = ModInfo::parse("[mod]\nid=\"showcase\"\n").unwrap();
        assert!(info.compat.is_none());
        assert!(check_compat(&info).is_ok());
    }

    #[test]
    fn loads_avast_bundle() {
        let dir = std::env::temp_dir().join("avast_loader_test");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("showcase.avast");
        let file = std::fs::File::create(&path).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        zip.start_file("mod.cfg", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"[mod]\nid=\"showcase\"\nversion=\"1.0.0\"\n")
            .unwrap();
        zip.finish().unwrap();
        let cfg = dir.join("showcase.toml");
        let r#mod =
            Mods::read_mod_from_avast_file(&path, &cfg, PackConfig::default()).unwrap();
        assert_eq!(r#mod.info.id, "showcase");
        assert!(r#mod.pack.is_none());
        assert_eq!(r#mod.root_directory, Some(path.clone()));
        std::fs::remove_file(&path).ok();
        std::fs::remove_file(&cfg).ok();
    }
}
