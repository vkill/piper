//! Async pipes, channels, mutexes, and more.
//!
//! **NOTE:** This crate is still a work in progress. Coming soon.

#![warn(missing_docs, missing_debug_implementations, rust_2018_idioms)]

mod arc;
mod chan;
mod event;
mod lock;
mod mutex;
mod pipe;

pub use arc::Arc;
pub use chan::{chan, Receiver, Sender};
pub use lock::{Lock, LockGuard};
pub use mutex::{Mutex, MutexGuard};
pub use pipe::{pipe, Reader, Writer};
