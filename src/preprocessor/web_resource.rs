//! The `web-resource` preprocessor

use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use serde::Deserialize;
use tokio::fs;
use tokio::task::JoinSet;
use tokio::io::AsyncWriteExt;

use crate::args::ARGS;
use crate::config;
use crate::query::Query;

use super::{BoxedPreprocessor, Preprocessor, PreprocessorDefinition};

/// Auxilliary configuration for the preprocessor
#[derive(Deserialize, Debug, Clone)]
pub struct Config {}

/// A resource that should be downloaded
#[derive(Deserialize, Debug, Clone)]
pub struct Resource {
    /// The URL to download from
    pub url: String,
    /// The path to download to. Must be in the document's root.
    pub path: PathBuf,
}

type QueryData = Vec<Resource>;

/// The `web-resource` preprocessor
pub struct WebResource {
    name: String,
    _config: Config,
    query: Query,
}

impl WebResource {
    async fn query(&self) -> Result<QueryData> {
        self.query.query().await
    }

    async fn download(resource: Resource) -> Result<()> {
        let Resource { url, path } = resource;

        let path = ARGS.resolve(&path)
        .with_context(|| {
            let path_str = path.to_string_lossy();
            format!("cannot download to {path_str} because it is outside the project root")
        })?;

        let path_str = path.to_string_lossy();
        println!("Downloading {url} to {path_str}...");

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let mut response = reqwest::get(url).await?;
        let mut file = fs::File::create(path).await?;
        while let Some(chunk) = response.chunk().await? {
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(())
    }
}

#[async_trait]
impl Preprocessor for WebResource {
    fn name(&self) -> &str {
        &self.name
    }

    async fn run(&mut self) -> Result<()> {
        let query_data = self.query().await?;

        let mut set = JoinSet::new();
        for resource in query_data {
            set.spawn(Self::download(resource));
        }

        while let Some(_) = set.join_next().await {
            // we just want to join all the tasks
        }

        Ok(())
    }
}

/// The `web-resource` preprocessor factory
pub struct WebResourceFactory;

impl WebResourceFactory {
    fn parse_config(config: toml::Table) -> Result<Config> {
        let config = config.try_into()
            .context("invalid web-resource configuration")?;
        Ok(config)
    }

    fn build_query(config: config::Query) -> Result<Query> {
        let config = Query::builder()
            .default_field(Some("value".to_string()))
            .default_one(false)
            .default_selector("<web-resource>".to_string())
            .build(config)?;
        if config.one {
            return Err(anyhow!("web-resource prequery does not support --one"));
        }

        Ok(config)
    }
}

impl PreprocessorDefinition for WebResourceFactory {
    const NAME: &'static str = "web-resource";

    fn configure(
        name: String,
        config: toml::Table,
        query: config::Query,
    ) -> Result<BoxedPreprocessor> {
        let _config = Self::parse_config(config)?;
        let query = Self::build_query(query)?;
        let instance = WebResource { name, _config, query };
        Ok(Box::new(instance))
    }
}
