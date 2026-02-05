//! Example data configuration loaded via OrthoConfig helpers.

use std::path::PathBuf;

use ortho_config::declarative::{LayerComposition, MergeComposer, MergeLayer, merge_value};
use ortho_config::discovery::{ConfigDiscovery, DiscoveryLayersOutcome};
use ortho_config::figment::Figment;
use ortho_config::{
    CsvEnv, OrthoConfig, OrthoJsonMergeExt, OrthoMergeExt, OrthoResult, sanitize_value,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const APP_NAME: &str = "example_data";
const CONFIG_ENV_VAR: &str = "EXAMPLE_DATA_CONFIG_PATH";
const DEFAULT_SEED_NAME: &str = "mossy-owl";
const DOTFILE_NAME: &str = ".example_data.toml";
const ENV_PREFIX: &str = "EXAMPLE_DATA_";

fn default_registry_path() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let direct = cwd.join("fixtures").join("example-data").join("seeds.json");
    if direct.is_file() {
        return direct;
    }

    let nested = cwd
        .join("backend")
        .join("fixtures")
        .join("example-data")
        .join("seeds.json");
    if nested.is_file() {
        return nested;
    }

    direct
}

/// Configuration values controlling example data seeding at startup.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ExampleDataSettings {
    /// Enable example data seeding on startup.
    #[serde(default)]
    pub is_enabled: bool,
    /// Seed name to load from the registry.
    pub seed_name: Option<String>,
    /// Optional override for the number of users generated.
    #[serde(alias = "user_count")]
    pub count: Option<usize>,
    /// Optional registry path override.
    pub registry_path: Option<PathBuf>,
}

impl ExampleDataSettings {
    /// Report whether example data seeding is enabled.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::example_data::ExampleDataSettings;
    ///
    /// let settings = ExampleDataSettings {
    ///     is_enabled: false,
    ///     seed_name: None,
    ///     count: None,
    ///     registry_path: None,
    /// };
    /// assert!(!settings.is_enabled());
    /// ```
    pub fn is_enabled(&self) -> bool {
        self.is_enabled
    }

    /// Return the configured seed name, falling back to the default.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use backend::example_data::ExampleDataSettings;
    ///
    /// let settings = ExampleDataSettings {
    ///     is_enabled: false,
    ///     seed_name: Some("rainbow-fox".to_string()),
    ///     count: None,
    ///     registry_path: None,
    /// };
    /// assert_eq!(settings.seed_name(), "rainbow-fox");
    /// ```
    pub fn seed_name(&self) -> &str {
        self.seed_name.as_deref().unwrap_or(DEFAULT_SEED_NAME)
    }

    /// Return the configured registry path, falling back to the default.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::path::PathBuf;
    ///
    /// use backend::example_data::ExampleDataSettings;
    ///
    /// let settings = ExampleDataSettings {
    ///     is_enabled: false,
    ///     seed_name: None,
    ///     count: None,
    ///     registry_path: Some(PathBuf::from("/tmp/example_registry.json")),
    /// };
    /// assert_eq!(
    ///     settings.registry_path(),
    ///     PathBuf::from("/tmp/example_registry.json")
    /// );
    /// ```
    pub fn registry_path(&self) -> PathBuf {
        self.registry_path
            .clone()
            .unwrap_or_else(default_registry_path)
    }

    fn merge_from_layers(layers: Vec<MergeLayer<'static>>) -> OrthoResult<Self> {
        let mut buffer = Value::Object(Map::new());
        for layer in layers {
            merge_value(&mut buffer, layer.into_value());
        }
        serde_json::from_value(buffer).into_ortho_merge_json()
    }
}

impl OrthoConfig for ExampleDataSettings {
    fn load_from_iter<I, T>(_iter: I) -> OrthoResult<Self>
    where
        I: IntoIterator<Item = T>,
        T: Into<std::ffi::OsString> + Clone,
    {
        let composition = compose_layers();
        composition.into_merge_result(Self::merge_from_layers)
    }

    fn prefix() -> &'static str {
        ENV_PREFIX
    }
}

fn compose_layers() -> LayerComposition {
    let mut errors = Vec::new();
    let mut composer = MergeComposer::with_capacity(3);

    let defaults = ExampleDataSettings {
        is_enabled: false,
        seed_name: None,
        count: None,
        registry_path: None,
    };
    match sanitize_value(&defaults) {
        Ok(value) => composer.push_defaults(value),
        Err(err) => errors.push(err),
    }

    let discovery = ConfigDiscovery::builder(APP_NAME)
        .env_var(CONFIG_ENV_VAR)
        .dotfile_name(DOTFILE_NAME)
        .build();
    let DiscoveryLayersOutcome {
        value: layers,
        mut required_errors,
        mut optional_errors,
    } = discovery.compose_layers();
    errors.append(&mut required_errors);
    if layers.is_empty() {
        errors.append(&mut optional_errors);
    }
    for layer in layers {
        composer.push_layer(layer);
    }

    let env_provider = CsvEnv::prefixed(ENV_PREFIX)
        .map(|key| ortho_config::uncased::Uncased::new(key.as_str().to_ascii_uppercase()))
        .split("__");
    match Figment::from(env_provider)
        .extract::<Value>()
        .into_ortho_merge()
    {
        Ok(value) => composer.push_environment(value),
        Err(err) => errors.push(err),
    }

    LayerComposition::new(composer.layers(), errors)
}

#[cfg(test)]
mod tests {
    //! Unit tests for example data configuration parsing.

    use super::*;
    use std::ffi::OsString;

    use env_lock::lock_env;
    use rstest::{fixture, rstest};

    struct SettingsLoader;

    impl SettingsLoader {
        fn load(&self) -> ExampleDataSettings {
            ExampleDataSettings::load_from_iter([OsString::from("backend")])
                .expect("config should load")
        }
    }

    #[fixture]
    fn load_settings_from_empty_args() -> SettingsLoader {
        SettingsLoader
    }

    #[rstest]
    fn default_values_are_used_when_missing(load_settings_from_empty_args: SettingsLoader) {
        let _guard = lock_env([
            ("EXAMPLE_DATA_IS_ENABLED", None::<String>),
            ("EXAMPLE_DATA_SEED_NAME", None::<String>),
            ("EXAMPLE_DATA_COUNT", None::<String>),
            ("EXAMPLE_DATA_REGISTRY_PATH", None::<String>),
        ]);

        let settings = load_settings_from_empty_args.load();
        assert!(!settings.is_enabled());
        assert_eq!(settings.seed_name(), DEFAULT_SEED_NAME);
        assert_eq!(settings.registry_path(), default_registry_path());
        assert!(settings.count.is_none());
    }

    #[rstest]
    fn environment_overrides_are_respected(load_settings_from_empty_args: SettingsLoader) {
        let _guard = lock_env([
            ("EXAMPLE_DATA_IS_ENABLED", Some("true".to_owned())),
            ("EXAMPLE_DATA_SEED_NAME", Some("rainbow-fox".to_owned())),
            ("EXAMPLE_DATA_COUNT", Some("5".to_owned())),
            (
                "EXAMPLE_DATA_REGISTRY_PATH",
                Some("/tmp/example_registry.json".to_owned()),
            ),
        ]);

        let settings = load_settings_from_empty_args.load();
        assert!(settings.is_enabled());
        assert_eq!(settings.seed_name(), "rainbow-fox");
        assert_eq!(
            settings.registry_path(),
            PathBuf::from("/tmp/example_registry.json")
        );
        assert_eq!(settings.count, Some(5));
    }
}
