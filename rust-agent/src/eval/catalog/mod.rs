//! Modular multi-source eval catalog: metadata, filter, seeded sample.
//!
//! Item **bodies** are never hard-coded in-repo. Each source has `remote.toml`
//! pointing at public online datasets; see [`remote`].

mod filter;
mod grade;
mod load;
mod remote;
mod run;
mod sample;
mod types;

pub use filter::{filter_items, matches_filter};
pub use grade::{grade_catalog_item, CatalogGrade};
pub use load::{
    catalog_root, list_source_summaries, load_all_sources, load_catalog_items, load_items,
    load_source_meta, sources_dir,
};
pub use run::{run_catalog_sample, CatalogRunOpts, CatalogRunReport};
pub use sample::sample_items;
pub use types::{CatalogFilter, CatalogItem, GradeSpec, SourceMeta, SourceSummary};
