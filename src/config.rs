//! Configuration types

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result};
use serde::{Deserialize, Deserializer};
use serde::de::{self, Visitor};
use tokio::fs;
use toml::Table;

/// The complete prequery config as found in the `[tool.prequery]` section in `typst.toml`. Usually,
/// that section will be defined as multiple `[[tool.prequery.jobs]]` entries.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Config {
    /// The preprocessing jobs to execute
    pub jobs: Vec<Job>,
}

/// A single preprocessing job. A job normally consists of executing the configured query and then
/// processing the result in some way, usually writing to files in the project root.
#[derive(Deserialize, Debug, Clone, PartialEq)]
pub struct Job {
    /// The job's name (for human consumption, e.g. in logs)
    pub name: String,
    /// Identifier of the preprocessor that should be run
    pub kind: String,
    /// The query the preprocessor needs to run
    pub query: Query,
    /// Arbitrary additional configuration that is available to the job
    #[serde(flatten)]
    pub config: Table,
}

/// Query configuration. All fields here are optional, as preprocessors can define their own
/// defaults.
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// The selector to be queried, e.g. `<label>`
    pub selector: Option<String>,
    /// The field (`--field`) to be queried from the selector (with metadata elements, this is
    /// usually `value`)
    #[serde(default, deserialize_with = "deserialize_field")]
    pub field: Option<Option<String>>,
    /// Whether only one (`--one`) query result is expected and should be returned
    pub one: Option<bool>,
    /// Any additional inputs (`--input`) to be given to the queried document. Regardless of these
    /// settings, `prequery-fallback` is always set to `true` during queries.
    #[serde(default)]
    pub inputs: HashMap<String, String>,
}

impl Config {
    /// Given the contents of a `typst.toml` file, parses the `[tool.prequery]` section.
    pub fn parse(content: &str) -> Result<Self> {
        let mut config = toml::Table::from_str(content)?;
        let config = config
            .remove("tool")
            .context("typst.toml does not contain `tool` section")?
            .try_into::<Table>()
            .context("typst.toml contains `tool` key, but it's not a table")?
            .remove("prequery")
            .context("typst.toml does not contain `tool.prequery` section")?
            .try_into::<Self>()
            .context("typst.toml contains `tool.prequery` key, but it's not a valid preprocessor configuration")?;
        Ok(config)
    }

    /// Resolves and reads the given `typst.toml` file.
    pub async fn read<P: AsRef<Path>>(path: P) -> Result<Self> {
        let config = fs::read_to_string(path).await?;
        let config = Self::parse(&config)?;
        Ok(config)
    }
}

/// Deserializes the `field` config: if given, must be either a string or `false`.
fn deserialize_field<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>
{
    struct FieldVisitor;

    impl<'de> Visitor<'de> for FieldVisitor {
        type Value = Option<Option<String>>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("`false` or a string`")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v {
                return Err(E::invalid_value(de::Unexpected::Bool(v), &self));
            }
            Ok(Some(None))
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            self.visit_string(v.to_owned())
        }

        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(Some(v)))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(FieldVisitor)
}
