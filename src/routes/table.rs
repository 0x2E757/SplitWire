use std::io;

use crate::net::Ipv4Net;

/// The OS route table, seen only through the routes SplitWire owns on one adapter at a time.
pub trait RouteTable: Send + Sync {
    /// The routes SplitWire owns on the adapter `luid` that are present now.
    fn list_owned(&self, luid: u64) -> io::Result<Vec<Ipv4Net>>;
    fn add(&self, luid: u64, net: Ipv4Net) -> io::Result<()>;
    fn remove(&self, luid: u64, net: Ipv4Net) -> io::Result<()>;
}
