//! IP Helper: the routes on a profile's adapter (`IpHelperRouteTable`), the adapter's address,
//! MTU, metric and DNS settings (`IpHelperAddressing`), and the read-only interface facts
//! (`IpHelperProbe`). Tests build the rows and make read-only calls only; the writes run only
//! in live tests, which a person runs.

mod abi;
mod addressing;
mod probe;
mod route_table;

pub use addressing::IpHelperAddressing;
pub use probe::IpHelperProbe;
pub use route_table::IpHelperRouteTable;
