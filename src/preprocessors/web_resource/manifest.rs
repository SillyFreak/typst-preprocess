use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer};

use crate::args::ARGS;

/// Auxilliary configuration for the preprocessor
#[derive(Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// Always downloads and overwrites all files. It is not recommended to permanently set this
    /// option, but temporarily enabling it can make sense to check for changed resources.
    #[serde(default)]
    pub overwrite: bool,

    /// Change this to true or a file path given as a string to enable the index. If true, the
    /// default path is "web-resource-index.toml"; note that if multiple web-resource jobs are using
    /// the same index file, this will lead to problems!
    #[serde(default, deserialize_with = "deserialize_index")]
    pub index: Option<PathBuf>,

    /// Change this to true to delete files no longer needed by the document this requires the index
    /// to be enabled.
    #[serde(default)]
    pub evict: bool,
}

impl Manifest {
    pub async fn resolve_index_path(&self) -> Option<io::Result<PathBuf>> {
        async fn inner<P: AsRef<Path>>(index: P) -> io::Result<PathBuf> {
            let mut path = ARGS.resolve_typst_toml().await?;
            let result = path.pop();
            assert!(
                result,
                "the path should have had a final filename component"
            );
            path.push(&index);
            Ok(path)
        }

        if let Some(index) = &self.index {
            Some(inner(index).await)
        } else {
            None
        }
    }
}

/// Deserializes the `index` config: if given, must be either a boolean or string.
fn deserialize_index<'de, D>(deserializer: D) -> Result<Option<PathBuf>, D::Error>
where
    D: Deserializer<'de>,
{
    struct IndexVisitor;

    impl<'de> Visitor<'de> for IndexVisitor {
        type Value = Option<PathBuf>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a boolean or string`")
        }

        fn visit_bool<E>(self, v: bool) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(v.then(|| "web-resource-index.toml".into()))
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
            Ok(Some(v.into()))
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }
    }

    deserializer.deserialize_any(IndexVisitor)
}
