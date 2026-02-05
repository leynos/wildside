//! Example data configuration loaded via OrthoConfig.

use std::path::PathBuf;

use ortho_config::OrthoConfig;
use serde::{Deserialize, Serialize};

const DEFAULT_SEED_NAME: &str = "mossy-owl";

fn default_registry_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("example-data")
        .join("seeds.json")
}

/// Configuration values controlling example data seeding at startup.
#[derive(Debug, Clone, Deserialize, Serialize, OrthoConfig)]
#[ortho_config(prefix = "EXAMPLE_DATA_")]
pub struct ExampleDataSettings {
    /// Enable example data seeding on startup.
    #[serde(default)]
    #[ortho_config(default = false, cli_default_as_absent)]
    pub is_enabled: bool,
    /// Seed name to load from the registry.
    pub seed_name: Option<String>,
    /// Optional override for the number of users generated.
    #[ortho_config(file_key = "user_count")]
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
