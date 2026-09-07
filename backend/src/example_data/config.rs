//! Example data configuration loaded via OrthoConfig helpers.

use std::path::PathBuf;

use ortho_config::declarative::{LayerComposition, MergeComposer, MergeLayer, merge_value};
use ortho_config::discovery::{ConfigDiscovery, DiscoveryLayersOutcome};
use ortho_config::figment::{Figment, Provider};
use ortho_config::uncased::{Uncased, UncasedStr};
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

/// Locate the bundled seed registry relative to the current directory.
///
/// Tries `fixtures/` and then `backend/fixtures/` so the binary works whether
/// it runs from the workspace root or from the backend crate.
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

    /// Load the settings from the defaults, any discovered configuration
    /// files, and an explicit environment layer.
    ///
    /// This is the whole of [`OrthoConfig::load_from_iter`] apart from
    /// constructing the provider, so a test that calls it exercises the same
    /// composition and merge path the application uses (#464).
    fn load_with_provider(env_provider: impl Provider) -> OrthoResult<Self> {
        compose_layers(env_provider).into_merge_result(Self::merge_from_layers)
    }

    /// Merge the composed layers, lowest precedence first, into the settings.
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
        Self::load_with_provider(process_env_provider())
    }

    fn prefix() -> &'static str {
        ENV_PREFIX
    }
}

/// The `EXAMPLE_DATA_`-prefixed process environment, as a Figment provider.
///
/// This is the only place the settings loader touches the environment;
/// [`compose_layers`] takes the provider as an argument so its merge
/// precedence can be exercised against an explicit layer instead (#464).
fn process_env_provider() -> impl Provider {
    CsvEnv::prefixed(ENV_PREFIX)
        .map(uppercase_env_key)
        .split("__")
}

/// Normalize an environment variable name to the upper-case form the layer
/// keys on.
///
/// Named rather than inlined so the mapping the provider applies can be
/// asserted without reading the process.
fn uppercase_env_key(key: &UncasedStr) -> Uncased<'_> {
    Uncased::new(key.as_str().to_ascii_uppercase())
}

/// Compose the defaults, any discovered configuration files, and the supplied
/// environment layer, in ascending order of precedence.
///
/// Errors from each stage are collected rather than short-circuiting, so a
/// caller sees every problem at once.
fn compose_layers(env_provider: impl Provider) -> LayerComposition {
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
    use std::error::Error as StdError;

    use ortho_config::figment::providers::Serialized;
    use rstest::rstest;
    use serde_json::json;

    type TestResult<T = ()> = Result<T, Box<dyn StdError>>;

    /// Load the settings from an explicit environment layer.
    ///
    /// `load_with_provider` is the whole of `load_from_iter` apart from
    /// constructing the provider, so these cases exercise the real defaults,
    /// discovery, and merge path with the process read replaced by a layer
    /// (#464).
    fn load_with_env_layer(env_layer: Value) -> TestResult<ExampleDataSettings> {
        Ok(ExampleDataSettings::load_with_provider(
            Serialized::defaults(env_layer),
        )?)
    }

    /// Scenario: no `EXAMPLE_DATA_*` value reaches the loader.
    ///
    /// Invariant: every setting falls back to its documented default.
    #[rstest]
    fn default_values_are_used_when_missing() -> TestResult {
        let settings = load_with_env_layer(json!({}))?;

        assert!(!settings.is_enabled());
        assert_eq!(settings.seed_name(), DEFAULT_SEED_NAME);
        assert_eq!(settings.registry_path(), default_registry_path());
        assert!(settings.count.is_none());
        Ok(())
    }

    /// Scenario: every setting is supplied through the environment layer.
    ///
    /// Invariant: the environment layer wins over the defaults layer for each
    /// field.
    #[rstest]
    fn environment_overrides_are_respected() -> TestResult {
        let settings = load_with_env_layer(json!({
            "is_enabled": true,
            "seed_name": "rainbow-fox",
            "count": 5,
            "registry_path": "/tmp/example_registry.json",
        }))?;

        assert!(settings.is_enabled());
        assert_eq!(settings.seed_name(), "rainbow-fox");
        assert_eq!(
            settings.registry_path(),
            PathBuf::from("/tmp/example_registry.json")
        );
        assert_eq!(settings.count, Some(5));
        Ok(())
    }

    /// Scenario: the environment layer supplies one field and omits the rest.
    ///
    /// Invariant: the supplied field overrides its default and the omitted
    /// ones keep theirs, so the layers merge field by field rather than
    /// wholesale.
    #[rstest]
    fn a_partial_environment_layer_overrides_only_its_own_fields() -> TestResult {
        let settings = load_with_env_layer(json!({ "seed_name": "rainbow-fox" }))?;

        assert_eq!(settings.seed_name(), "rainbow-fox");
        assert!(
            !settings.is_enabled(),
            "an unset toggle must keep its default rather than being cleared"
        );
        assert!(
            settings.count.is_none(),
            "an unset count must keep its default rather than being cleared"
        );
        Ok(())
    }

    /// Scenario: the environment layer carries a value of the wrong type.
    ///
    /// Invariant: loading reports an error rather than silently falling back
    /// to the default, so a malformed `EXAMPLE_DATA_COUNT` is not mistaken for
    /// an absent one.
    #[rstest]
    fn a_malformed_value_fails_the_load() {
        let outcome = load_with_env_layer(json!({ "count": "not-a-number" }));

        assert!(
            outcome.is_err(),
            "a non-numeric count must fail the load, not fall back to the default"
        );
    }

    /// Scenario: the provider maps a lower-case variable name.
    ///
    /// Invariant: the key is upper-cased, which is what lets the
    /// `EXAMPLE_DATA_` prefix and the `__` nesting separator match regardless
    /// of how the variable was spelled.
    #[rstest]
    #[case::lower_case("example_data_seed_name", "EXAMPLE_DATA_SEED_NAME")]
    #[case::mixed_case("Example_Data__Seed_Name", "EXAMPLE_DATA__SEED_NAME")]
    #[case::already_upper_case("EXAMPLE_DATA_COUNT", "EXAMPLE_DATA_COUNT")]
    fn the_provider_upper_cases_environment_keys(#[case] key: &str, #[case] expected: &str) {
        assert_eq!(
            uppercase_env_key(UncasedStr::new(key)).as_str(),
            expected,
            "the provider must key on the upper-case form of the variable name"
        );
    }
}
