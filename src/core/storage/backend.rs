//! Stub; Task 1.6 replaces with the real StorageBackend trait.

#![allow(dead_code)]

use std::future::Future;
use std::pin::Pin;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

pub struct CommitOutcome;

pub trait StorageBackend {}
