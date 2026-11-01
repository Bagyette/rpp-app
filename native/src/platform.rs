//! Operating-system integration shared by the editor and exporters.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use directories::ProjectDirs;
use eframe::egui::FontData;
use resvg::usvg::fontdb::{Database, Family, Query, Weight};

fn project_dirs() -> Option<ProjectDirs> {
    // Empty organization/qualifier preserves the original Windows LocalAppData/RPP directory.
    ProjectDirs::from("", "", "RPP")
}

pub fn config_file(name: &str) -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.config_local_dir().join(name))
}

pub fn recovery_file() -> Option<PathBuf> {
    project_dirs().map(|dirs| dirs.data_local_dir().join("autosave.rpp"))
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
        assert_eq!(settings.parent(), recovery.parent());
        #[cfg(target_os = "macos")]
        assert!(
            settings
                .to_string_lossy()
                .contains("Library/Application Support/RPP")
        );
    }
}
