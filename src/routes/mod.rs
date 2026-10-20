//! One profile's routes on its adapter: the never-routes, the wanted and installed routes, the
//! learned hosts and their expiry, the refusals, and which profile holds each learned host.

mod guard;
mod host_claims;
mod learned;
mod manager;
mod planner;
mod refused;
mod table;

pub use guard::RouteGuard;
pub use host_claims::{ClaimSplit, HostClaims};
pub use learned::{LearnedRoute, LearnedRoutes};
pub use manager::{RouteManager, RoutesSnapshot, StaticRoute};
pub use planner::{RouteOrigin, RoutePlan, RoutePlanner};
pub use refused::{RefusedRoute, RefusedRoutes};
pub use table::RouteTable;
