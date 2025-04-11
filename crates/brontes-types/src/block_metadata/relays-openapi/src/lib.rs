#![allow(clippy::too_many_arguments)]
#![allow(clippy::doc_lazy_continuation)]
#![allow(clippy::empty_line_after_doc_comments)]

#[macro_use]
extern crate serde_derive;

extern crate reqwest;
extern crate serde;
extern crate serde_json;
extern crate url;

pub mod apis;
#[rustfmt::skip]
pub mod models;
