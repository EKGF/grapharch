//! A mock RDF4J SPARQL endpoint that serves persistent HTTP/1.1 connections
//! and counts connections and requests.
#![allow(dead_code)]
mod http;
mod query;
mod server;
mod state;

pub use server::Server;
