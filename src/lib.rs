//! Unofficial PagerDuty API Client
//!
//! The code for this project resides at https://github.com/jwilm/pagerduty-rs.
//!
//! This is an early version of the PagerDuty API Client. Many of the APIs are not yet implemented
//! in this client. The initial features implemented are those necessary to create new incidents
//! from a monitoring service. Furthemore, this library only works on Rust _nightlies_ at the
//! moment; we are using the Serde library for handling JSON serialization and make extensive use of
//! the automatically derived `De/Serialize` implementations.
//!
//! # Support
//!
//! The following APIs are **supported**
//!
//! * Integration API
//!
//! The following APIs are **unsupported**
//!
//! * Alerts
//! * Escalation Policies
//! * Incidents
//! * Log Entries
//! * Maintenance Windows
//! * Reports
//! * Schedules
//! * Services
//! * Users
//! * Teams
//!
//! Additionally, the following features are unsupported
//!
//! * Webhooks
//!
//! If you are interested in using this library and the feature you want is not yet implemented,
//! please file an issue on this project's repository. Features will be implemented on a
//! most-in-demand basis.
//!
//! # Tips
//!
//! There are a few things to know that might ease getting started with this library.
//!
//! * Request types store string values as `Cow<'a, str>`, and setters for these properties accept
//! `Into<Cow<'a, str>>` to keep the API ergononmic.
//! * Parts of the API (specifically, integration event `set_details`) let you provide arbitrary
//! data with the request. Any structured type that implements `Serialize` can be used in these
//! cases. There's currently no enforcement of the _structured_ part of that contract. If you do not
//! uphold that constaint, you will probably get a `BadRequest` response.
//!
#![feature(custom_derive, plugin)]
#![plugin(serde_macros)]

extern crate hyper;
extern crate serde;
extern crate serde_json;

pub mod integration;

mod auth;
pub use auth::*;

mod request;
