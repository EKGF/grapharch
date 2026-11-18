//! A website is an arrangement of selected logical fragments, with page and
//! navigation metadata read exclusively from the pinned documentation graph.
mod artifact;
mod assets;
mod budget;
mod catalog;
mod gallery;
mod header;
mod header_links;
mod interactive_svg;
mod markdown;
mod metadata;
mod navigation;
mod page;
mod project;
mod redirects;
mod report;
mod repository;
mod search;
mod shell;
mod site;
mod site_base;
mod site_chrome;
mod site_link;
mod theme;

pub(super) use {budget::MAX_ARTIFACTS, site::write};
