//! HTML documentation generator (`pkldoc`) for Apple Pkl 0.32.0.
//!
//! Provides byte-for-byte HTML, JSON, search-index, static web asset, symlink,
//! and v1-to-v2 site migration parity with `org.pkl.doc`.

pub mod assets;
pub mod cli;
pub mod generator;
pub mod html;
pub mod json;
pub mod markdown;
pub mod migrator;
pub mod model;
pub mod package_data;
pub mod pages;
pub mod runtime_data;
pub mod scope;
pub mod search_index;
pub mod util;

pub use cli::{
    CliDocGenerator, CliDocGeneratorOptions, PKLDOC_HELP_TEXT, pkldoc_version_info, run_cli,
    run_cli_with_io,
};
pub use generator::{CURRENT_DIRECTORY_NAME, DocGenerator};
pub use migrator::DocMigrator;
pub use model::{
    DocGeneratorError, DocModule, DocPackage, DocPackageInfo, DocsiteInfo, PackageDependency,
};
