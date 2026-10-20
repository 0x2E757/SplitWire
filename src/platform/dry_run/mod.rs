//! Stand-ins that log what would be done and change nothing: the driver, the adapter's
//! addressing, the routes, NRPT, the DNS cache flush, the bound upstream exchange (over
//! loopback) and the data folder's ACL. `run --dry-run` and the engine tests use them.

mod addressing;
mod driver;
mod host;
mod nrpt;
mod route_table;

pub use addressing::DryRunAddressing;
pub use driver::DryRunDriver;
pub use host::{DryRunDnsFlush, DryRunExchangeFactory, DryRunFolderAccess};
pub use nrpt::DryRunNrpt;
pub use route_table::DryRunRouteTable;
