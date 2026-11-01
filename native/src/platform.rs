//! Operating-system integration shared by the editor and exporters.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

#[cfg(target_os = "windows")]
use directories::BaseDirs;
#[cfg(not(target_os = "windows"))]
use directories::ProjectDirs;
use eframe::egui::FontData;
use resvg::usvg::fontdb::{Database, Family, Query, Weight};

struct StorageDirectories {
    config: PathBuf,
    recovery: PathBuf,
}

fn storage_directories() -> Option<StorageDirectories> {
    #[cfg(target_os = "windows")]
    {
        // ProjectDirs adds config/data subfolders on Windows. Preserve the original
        // flat LocalAppData/RPP layout so upgrades retain settings and recovery.
        let directory = BaseDirs::new()?.data_local_dir().join("RPP");
        Some(StorageDirectories {
            config: directory.clone(),
            recovery: directory,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let directories = ProjectDirs::from("", "", "RPP")?;
        Some(StorageDirectories {
            config: directories.config_local_dir().to_path_buf(),
            recovery: directories.data_local_dir().to_path_buf(),
        })
    }
}

pub fn config_file(name: &str) -> Option<PathBuf> {
    storage_directories().map(|directories| directories.config.join(name))
}

pub fn recovery_file() -> Option<PathBuf> {
    storage_directories().map(|directories| directories.recovery.join("autosave.rpp"))
}

pub fn font_database() -> Arc<Database> {
    static DATABASE: OnceLock<Arc<Database>> = OnceLock::new();
    Arc::clone(DATABASE.get_or_init(|| {
        let mut database = Database::new();
        database.load_system_fonts();
        for font in eframe::egui::FontDefinitions::default().font_data.values() {
            database.load_font_data(font.font.to_vec());
        }
        // Keep text exports usable on machines without Arial (especially Linux).
        let families = [
            "Arial",
            "Liberation Sans",
            "DejaVu Sans",
            "Helvetica",
            "Ubuntu",
        ];
        for family in families {
            if database
                .query(&Query {
                    families: &[Family::Name(family)],
                    ..Query::default()
                })
                .is_some()
            {
                database.set_sans_serif_family(family);
                break;
            }
        }
        Arc::new(database)
    }))
}

pub fn editor_font(weight: Weight) -> Option<FontData> {
    let database = font_database();
    let face = database.query(&Query {
        families: &[
            Family::Name("Arial"),
            Family::Name("Liberation Sans"),
            Family::Name("DejaVu Sans"),
            Family::SansSerif,
        ],
        weight,
        ..Query::default()
    })?;
    database.with_face_data(face, |bytes, index| {
        let mut font = FontData::from_owned(bytes.to_vec());
        font.index = index;
        font
    })
}

#[cfg(test)]
mod tests {
    use super::{config_file, recovery_file};

    #[test]
    fn storage_paths_are_absolute_and_preserve_filenames() {
        let settings = config_file("settings.json").expect("user configuration directory");
        let recovery = recovery_file().expect("user data directory");
        assert!(settings.is_absolute());
        assert!(recovery.is_absolute());
        assert_eq!(settings.file_name().expect("filename"), "settings.json");
        assert_eq!(recovery.file_name().expect("filename"), "autosave.rpp");
        #[cfg(target_os = "windows")]
        {
            let legacy = directories::BaseDirs::new()
                .expect("Windows user directories")
                .data_local_dir()
                .join("RPP");
            assert_eq!(settings.parent(), Some(legacy.as_path()));
            assert_eq!(recovery.parent(), Some(legacy.as_path()));
        }
        #[cfg(target_os = "macos")]
        assert!(
            settings
                .to_string_lossy()
                .contains("Library/Application Support/RPP")
        );
    }
}
