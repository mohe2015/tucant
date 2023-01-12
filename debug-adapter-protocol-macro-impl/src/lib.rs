// SPDX-FileCopyrightText: The tucant Contributors
//
// SPDX-License-Identifier: AGPL-3.0-or-later
//#![deny(unused_results)]
#![warn(clippy::pedantic, clippy::nursery, clippy::cargo)]
#![allow(
    clippy::missing_panics_doc,
    clippy::missing_errors_doc,
    clippy::multiple_crate_versions
)]
#![feature(array_try_map)]

pub mod debug_adapter_protocol;
pub mod json_parser;
pub mod json_schema;
pub mod json_schema_codegen;
mod utils;

// https://datatracker.ietf.org/doc/html/draft-fge-json-schema-validation-00
