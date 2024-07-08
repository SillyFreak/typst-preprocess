//! The `web-resource` preprocessor

use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use tokio::fs;
use tokio::task::JoinSet;
use tokio::io::AsyncWriteExt;

use crate::args::ARGS;
use crate::config:: Query as ConfigQuery;
use crate::query::Query;

use super::{BoxedPreprocessor, Preprocessor, PreprocessorDefinition};

mod config;

use config::*;

/// The `web-resource` preprocessor
pub struct WebResource {
    name: String,
    config: Arc<Config>,
    query: Query,
}

impl WebResource {
    async fn query(&self) -> Result<QueryData> {
        self.query.query().await
    }

    async fn download(config: Arc<Config>, resource: Resource) -> Result<()> {
        let Resource { url, path } = resource;

        let path = ARGS.resolve(&path)
        .with_context(|| {
            let path_str = path.to_string_lossy();
            format!("cannot download to {path_str} because it is outside the project root")
        })?;
        let path_str = path.to_string_lossy();

        let exists = fs::try_exists(&path).await.unwrap_or(false);
        let download = if !exists {
            println!("Downloading {url} to {path_str}...");
            true
        } else if config.overwrite {
            println!("Downloading {url} to {path_str} (overwrite of existing files was forced)...");
            true
        } else if let Some(index) = config.resolve_index_path().await {
            let index = index?;
            // TODO check whether the URL in the index is the same as the one in the typst file
            println!("Downloading {url} to {path_str}...");
            true
        } else {
            println!("Downloading of {url} to {path_str} skipped (file exists)...");
            false
        };

        if download {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }

            let mut response = reqwest::get(url).await?;
            let mut file = fs::File::create(path).await?;
            while let Some(chunk) = response.chunk().await? {
                file.write_all(&chunk).await?;
            }
            file.flush().await?;
        }

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
            set.spawn(Self::download(self.config.clone(), resource));
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

    fn build_query(config: ConfigQuery) -> Result<Query> {
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
        query: ConfigQuery,
    ) -> Result<BoxedPreprocessor> {
        let config = Arc::new(Self::parse_config(config)?);
        let query = Self::build_query(query)?;
        let instance = WebResource { name, config, query };
        Ok(Box::new(instance))
    }
}
