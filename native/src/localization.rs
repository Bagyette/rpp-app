use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
struct Catalog {
    default_language: String,
    languages: BTreeMap<String, Language>,
}

#[derive(Deserialize)]
struct Language {
    name: String,
    strings: BTreeMap<String, String>,
}

pub struct Localizer {
    catalog: Catalog,
    current_language: String,
}

impl Localizer {
    pub fn load() -> Self {
        let catalog: Catalog =
            serde_yaml::from_str(include_str!("../lang.yaml")).expect("valid language catalog");
        let current_language = catalog.default_language.clone();
        Self {
            catalog,
            current_language,
        }
    }

    pub fn current_language(&self) -> &str {
        &self.current_language
    }

    pub fn set_language(&mut self, language: &str) {
        if self.catalog.languages.contains_key(language) {
            self.current_language = language.to_owned();
        }
    }

    pub fn languages(&self) -> Vec<(String, String)> {
        self.catalog
            .languages
            .iter()
            .map(|(code, language)| (code.clone(), language.name.clone()))
            .collect()
    }

    pub fn text(&self, key: &str) -> String {
        self.catalog
            .languages
            .get(&self.current_language)
            .and_then(|language| language.strings.get(key))
            .or_else(|| {
                self.catalog
                    .languages
                    .get(&self.catalog.default_language)
                    .and_then(|language| language.strings.get(key))
            })
            .cloned()
            .unwrap_or_else(|| key.to_owned())
    }

    pub fn format(&self, key: &str, values: &[(&str, String)]) -> String {
        let mut output = self.text(key);
        for (name, value) in values {
            output = output.replace(&format!("{{{name}}}"), value);
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::Localizer;

    #[test]
    fn defaults_to_us_english() {
        let localizer = Localizer::load();
        assert_eq!(localizer.current_language(), "en-US");
        assert_eq!(localizer.text("action.save"), "Save");
    }

    #[test]
    fn every_language_has_identical_keys() {
        let localizer = Localizer::load();
        let english = &localizer.catalog.languages["en-US"].strings;
        let french = &localizer.catalog.languages["fr-FR"].strings;
        let spanish = &localizer.catalog.languages["es-ES"].strings;
        assert_eq!(
            english.keys().collect::<Vec<_>>(),
            french.keys().collect::<Vec<_>>()
        );
        assert_eq!(
            english.keys().collect::<Vec<_>>(),
            spanish.keys().collect::<Vec<_>>()
        );
    }

    #[test]
    fn missing_translation_falls_back_to_english() {
        let mut localizer = Localizer::load();
        localizer.set_language("fr-FR");
        localizer
            .catalog
            .languages
            .get_mut("fr-FR")
            .expect("French catalog")
            .strings
            .remove("action.save");
        assert_eq!(localizer.text("action.save"), "Save");
    }

    #[test]
    fn spanish_is_available_and_translates_core_tools() {
        let mut localizer = Localizer::load();
        localizer.set_language("es-ES");
        assert_eq!(localizer.text("action.save"), "Guardar");
        assert_eq!(localizer.text("tool.arrow"), "Flecha");
    }
}
