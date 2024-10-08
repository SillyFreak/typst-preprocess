//! Executing `typst query` commands

use std::collections::HashMap;
use std::fmt::Write;
use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command;

use crate::args::ARGS;
use crate::manifest;

pub use error::*;

/// A query that can be run against a Typst document. This is usually configured from a
/// [config::Query] using a [QueryBuilder].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    /// The selector to be queried, e.g. `<label>`
    pub selector: String,
    /// The field (`--field`) to be queried from the selector (with metadata elements, this is
    /// usually `value`)
    pub field: Option<String>,
    /// Whether only one (`--one`) query result is expected and should be returned
    pub one: bool,
    /// Any additional inputs (`--input`) to be given to the queried document. Regardless of these
    /// settings, `prequery-fallback` is always set to `true` during queries.
    pub inputs: HashMap<String, String>,
}

impl Query {
    /// Creates a query builder
    pub fn builder() -> QueryBuilder {
        QueryBuilder::default()
    }

    /// Builds the `typst query` command line for executing this command.
    pub fn command(&self) -> Command {
        let mut cmd = Command::new(&ARGS.typst);
        cmd.arg("query");
        if let Some(root) = &ARGS.root {
            cmd.arg("--root").arg(root);
        }
        if let Some(field) = &self.field {
            cmd.arg("--field").arg(field);
        }
        if self.one {
            cmd.arg("--one");
        }
        let mut input = String::new();
        for (key, value) in &self.inputs {
            input.clear();
            write!(&mut input, "{key}={value}").expect("writing to a string failed");
            cmd.arg("--input").arg(&input);
        }
        cmd.arg("--input").arg("prequery-fallback=true");
        cmd.arg(&ARGS.input).arg(&self.selector);

        cmd
    }

    /// Executes the query. This builds the necessary command line, runs the command, and returns
    /// the result parsed into the desired type from JSON.
    pub async fn query<T>(&self) -> Result<T>
    where
        T: for<'a> Deserialize<'a>,
    {
        let mut command = self.command();
        command.stderr(Stdio::inherit());
        let output = command.output().await?;
        if !output.status.success() {
            let status = output.status;
            Err(Error::Failure { command, status })?;
        }

        let value = serde_json::from_slice(&output.stdout)?;
        Ok(value)
    }
}

/// A query builder. Default values for the various configs can be set. If a setting is missing from
/// the [config::Query], that default will be used.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct QueryBuilder {
    /// The selector to be queried, e.g. `<label>`
    pub selector: Option<String>,
    /// The field (`--field`) to be queried from the selector (with metadata elements, this is
    /// usually `value`)
    pub field: Option<Option<String>>,
    /// Whether only one (`--one`) query result is expected and should be returned
    pub one: Option<bool>,
}

impl QueryBuilder {
    /// Set the selector to be queried, e.g. `<label>`
    pub fn default_selector(mut self, selector: String) -> Self {
        self.selector = Some(selector);
        self
    }

    /// Set the field (`--field`) to be queried from the selector (with metadata elements, this is
    /// usually `value`)
    pub fn default_field(mut self, field: Option<String>) -> Self {
        self.field = Some(field);
        self
    }

    /// Set whether only one (`--one`) query result is expected and should be returned
    pub fn default_one(mut self, one: bool) -> Self {
        self.one = Some(one);
        self
    }

    /// build a [Query] using the given defaults. If the [config::Query] doesn't contain a field
    /// that also doesn't have a default value, this will fail.
    pub fn build(self, config: manifest::Query) -> Result<Query, QueryBuilderError> {
        let selector = config
            .selector
            .or(self.selector)
            .ok_or(QueryBuilderError::Selector)?;
        let field = config
            .field
            .or(self.field)
            .ok_or(QueryBuilderError::Field)?;
        let one = config.one.or(self.one).ok_or(QueryBuilderError::One)?;
        let inputs = config.inputs;
        Ok(Query {
            selector,
            field,
            one,
            inputs,
        })
    }
}

mod error {
    use std::io;
    use std::process::ExitStatus;

    use thiserror::Error;
    use tokio::process::Command;

    /// Error while executing the query
    #[derive(Error, Debug)]
    pub enum Error {
        /// Reading command output failed
        #[error("reading from the `typst query` child process failed")]
        Io(#[from] io::Error),
        /// The subprocess failed
        #[error("query command failed: {status}\n\n\t{command:?}")]
        Failure {
            /// The command that was executed
            command: Command,
            /// The status code with which the command failed
            status: ExitStatus,
        },
        /// The response to the query was not valid
        #[error("query response was not valid JSON or did not fit the expected schema")]
        Json(#[from] serde_json::Error),
    }

    /// Error in the query builder: a required ocnfiguration is missing
    #[derive(Error, Debug)]
    pub enum QueryBuilderError {
        /// `selector` is missing
        #[error("`selector` was not specified but is required")]
        Selector,
        /// `field` is missing
        #[error("`field` was not specified but is required")]
        Field,
        /// `one` is missing
        #[error("`one` was not specified but is required")]
        One,
    }

    /// Result type alias that defaults error to [Error].
    pub type Result<T, E = Error> = std::result::Result<T, E>;
}
