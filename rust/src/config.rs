//! Configuration resolution: environment variables, defaults, and validation.
//!
//! Explicit options override environment variables, and environment
//! variables override defaults. Environment values are trimmed, and an empty
//! or whitespace-only value is ignored.

use std::fmt;
use std::time::Duration;

use crate::errors::Error;

/// The environment variable for the API key.
pub(crate) const API_KEY_ENV: &str = "TYPESAFE_API_KEY";
/// The environment variable for the base URL.
pub(crate) const BASE_URL_ENV: &str = "TYPESAFE_BASE_URL";
/// The environment variable for the default model.
pub(crate) const DEFAULT_MODEL_ENV: &str = "TYPESAFE_DEFAULT_MODEL";
/// The environment variable for the log level.
pub(crate) const LOG_LEVEL_ENV: &str = "TYPESAFE_LOG_LEVEL";
/// The environment variable for the OpenJEV API key.
pub(crate) const OPENJEV_API_KEY_ENV: &str = "OPENJEV_API_KEY";
/// The environment variable for provider selection ("openjev" forces OpenJEV).
pub(crate) const JEV_PROVIDER_ENV: &str = "JEV_PROVIDER";

/// The logging verbosity of the client.
///
/// Logging is off unless configured. At [`LogLevel::Info`], the client logs
/// one line per attempt result and per scheduled retry. At
/// [`LogLevel::Debug`], it also logs headers and bodies. Credential headers
/// are redacted; bodies are not.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    /// Request/response headers and bodies, attempt lines, and retries.
    Debug,
    /// One line per attempt result and per scheduled retry.
    Info,
    /// Only warnings and errors.
    Warn,
    /// Only errors.
    Error,
    /// No logging. The default.
    #[default]
    Off,
}

impl LogLevel {
    /// Parses a log level name, case-insensitively, ignoring surrounding
    /// whitespace. `warning` is accepted as an alias for `warn`.
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" | "warning" => Some(Self::Warn),
            "error" => Some(Self::Error),
            "off" => Some(Self::Off),
            _ => None,
        }
    }

    /// Resolves the level from an explicit setting or `TYPESAFE_LOG_LEVEL`.
    pub(crate) fn resolve(explicit: Option<Self>) -> Self {
        if let Some(level) = explicit {
            return level;
        }
        env_trimmed(LOG_LEVEL_ENV)
            .and_then(|value| Self::parse(&value))
            .unwrap_or_default()
    }
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Debug => "debug",
            Self::Info => "info",
            Self::Warn => "warn",
            Self::Error => "error",
            Self::Off => "off",
        };
        f.write_str(name)
    }
}

/// Reads an environment variable, trimmed; `None` when unset or blank.
pub(crate) fn env_trimmed(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Resolves a setting: explicit value, else environment, else `default`.
fn resolve_string(value: Option<&str>, env: &str, default: &str) -> String {
    if let Some(value) = value {
        return value.trim().to_owned();
    }
    env_trimmed(env).unwrap_or_else(|| default.to_owned())
}

/// Resolves and validates the API key: explicit value or environment, trimmed.
///
/// Rejects empty keys and keys containing whitespace, control characters, or
/// non-ASCII characters. The error never echoes the key.
pub(crate) fn resolve_api_key(api_key: Option<&str>) -> Result<String, Error> {
    resolve_api_key_with_env(api_key, API_KEY_ENV)
}

/// Like [`resolve_api_key`] but reads the key from `key_env` instead of the
/// fixed `TYPESAFE_API_KEY`. Used by the provider-selection logic to pick up
/// `OPENJEV_API_KEY` when OpenJEV is selected.
pub(crate) fn resolve_api_key_with_env(api_key: Option<&str>, key_env: &str) -> Result<String, Error> {
    let key = api_key
        .map(str::trim)
        .map(str::to_owned)
        .or_else(|| env_trimmed(key_env))
        .unwrap_or_default();
    if key.is_empty() {
        return Err(Error::Config(format!(
            "No API key was provided. Pass api_key or set the {key_env} environment variable."
        )));
    }
    if !key.is_ascii() {
        return Err(Error::Config(
            "API key must contain only printable ASCII characters without whitespace.".into(),
        ));
    }
    if key.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(Error::Config(
            "API key must contain only printable ASCII characters without whitespace.".into(),
        ));
    }
    if !key.chars().all(|c| {
        let c = c as u8;
        (0x21..=0x7e).contains(&c)
    }) {
        return Err(Error::Config(
            "API key must contain only printable ASCII characters without whitespace.".into(),
        ));
    }
    Ok(key)
}

/// Resolves the base URL, stripping a trailing `/`.
pub(crate) fn resolve_base_url(base_url: Option<&str>) -> String {
    resolve_base_url_with_default(base_url, crate::DEFAULT_BASE_URL)
}

/// Like [`resolve_base_url`] but uses `default` as the fallback default.
pub(crate) fn resolve_base_url_with_default(base_url: Option<&str>, default: &str) -> String {
    resolve_string(base_url, BASE_URL_ENV, default)
        .trim_end_matches('/')
        .to_owned()
}

/// Resolves the default model.
pub(crate) fn resolve_default_model(default_model: Option<&str>) -> String {
    resolve_default_model_with_default(default_model, crate::DEFAULT_MODEL)
}

/// Like [`resolve_default_model`] but uses `default` as the fallback default.
pub(crate) fn resolve_default_model_with_default(default_model: Option<&str>, default: &str) -> String {
    resolve_string(default_model, DEFAULT_MODEL_ENV, default)
}

/// Provider selection result: the key env var, default base URL, and default
/// model to use.
pub(crate) struct ProviderDefaults {
    pub key_env: &'static str,
    pub base_url: &'static str,
    pub model: &'static str,
}

/// Determines which provider (TypeSafe or OpenJEV) to use, based on
/// `JEV_PROVIDER`, the presence of a TypeSafe key, and the presence of an
/// OpenJEV key. See [`ProviderDefaults`].
///
/// Selection order: explicit `JEV_PROVIDER=openjev` wins; otherwise TypeSafe
/// if its key is set (explicit or env); otherwise OpenJEV if only
/// `OPENJEV_API_KEY` is set; otherwise TypeSafe defaults.
pub(crate) fn resolve_provider(explicit_api_key: Option<&str>) -> ProviderDefaults {
    if let Some(provider) = env_trimmed(JEV_PROVIDER_ENV) {
        if provider == "openjev" {
            return ProviderDefaults {
                key_env: OPENJEV_API_KEY_ENV,
                base_url: crate::OPENJEV_DEFAULT_BASE_URL,
                model: crate::OPENJEV_DEFAULT_MODEL,
            };
        }
    }
    if explicit_api_key.is_some_and(|k| !k.trim().is_empty())
        || env_trimmed(API_KEY_ENV).is_some()
    {
        return ProviderDefaults {
            key_env: API_KEY_ENV,
            base_url: crate::DEFAULT_BASE_URL,
            model: crate::DEFAULT_MODEL,
        };
    }
    if env_trimmed(OPENJEV_API_KEY_ENV).is_some() {
        return ProviderDefaults {
            key_env: OPENJEV_API_KEY_ENV,
            base_url: crate::OPENJEV_DEFAULT_BASE_URL,
            model: crate::OPENJEV_DEFAULT_MODEL,
        };
    }
    ProviderDefaults {
        key_env: API_KEY_ENV,
        base_url: crate::DEFAULT_BASE_URL,
        model: crate::DEFAULT_MODEL,
    }
}

/// Validates a per-attempt timeout: it must be greater than zero.
pub(crate) fn validate_timeout(timeout: Duration) -> Result<Duration, Error> {
    if timeout.is_zero() {
        return Err(Error::Config(
            "timeout must be a positive duration of time.".into(),
        ));
    }
    Ok(timeout)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Environment variables are process-global; these tests serialize on a
    // shared lock. The parent `tests::env_lock` module holds it across all
    // env-mutating tests (see tests/env.rs), and unit tests here use a local
    // copy of the same discipline by only reading well-known-unset variables.
    // Mutation-based tests live in tests/env.rs.

    #[test]
    fn parse_log_level() {
        assert_eq!(LogLevel::parse("debug"), Some(LogLevel::Debug));
        assert_eq!(LogLevel::parse("INFO"), Some(LogLevel::Info));
        assert_eq!(LogLevel::parse(" warn "), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("warning"), Some(LogLevel::Warn));
        assert_eq!(LogLevel::parse("error"), Some(LogLevel::Error));
        assert_eq!(LogLevel::parse("off"), Some(LogLevel::Off));
        assert_eq!(LogLevel::parse("nope"), None);
        assert_eq!(LogLevel::parse(""), None);
    }

    #[test]
    fn default_log_level_is_off() {
        assert_eq!(LogLevel::default(), LogLevel::Off);
    }

    #[test]
    fn api_key_validation() {
        // Missing/empty.
        let err = resolve_api_key(None).unwrap_err();
        assert!(matches!(err, Error::Config(_)));
        assert!(err.to_string().contains("No API key"));
        // Whitespace inside.
        for bad in ["key with space", "key\ttab", "key\nnewline", "ünïcödé"] {
            let err = resolve_api_key(Some(bad)).unwrap_err();
            assert!(matches!(err, Error::Config(_)));
            assert!(!err.to_string().contains(bad), "leaked key: {err}");
        }
        // Trimmed valid key.
        assert_eq!(
            resolve_api_key(Some("  sk-live-abc123  ")).unwrap(),
            "sk-live-abc123"
        );
    }

    #[test]
    fn timeout_validation() {
        assert!(validate_timeout(Duration::from_millis(1)).is_ok());
        let err = validate_timeout(Duration::ZERO).unwrap_err();
        assert!(matches!(err, Error::Config(_)));
    }

    #[test]
    fn base_url_strips_trailing_slash() {
        assert_eq!(
            resolve_base_url(Some("https://example.com/")),
            "https://example.com"
        );
        assert_eq!(
            resolve_base_url(Some("https://example.com//")),
            "https://example.com"
        );
    }
}
