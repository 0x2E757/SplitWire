//! The DNS wire format, the answer cache and forwarding to a set of servers; each connected
//! profile's resolver, its patience while the tunnel warms up, and the handler that sends a
//! query to the right one; the live policy and the NRPT rules. Blocking: each exchange waits
//! for its reply on the calling thread.

mod builder;
mod cache;
mod exchange;
mod forwarder;
mod nrpt;
mod policy;
mod profile_resolver;
mod query_handler;
mod upstream_warmup;
pub mod wire;

pub use builder::ResponseBuilder;
pub use cache::{CacheKey, CachedAnswer, DnsCache};
pub use exchange::{DnsCacheFlusher, DnsExchange, DnsExchangeFactory};
pub use forwarder::{Forwarder, ServerSet};
pub use nrpt::{NrptPolicyProbe, NrptRules, NrptSync};
pub use policy::{DnsPolicy, SharedPolicy};
pub use profile_resolver::{AnswerSource, ProfileResolver, Resolution, ResolverSet};
pub use query_handler::{ListedAnswer, QueryHandler, QueryHandlerParts, QueryOutcome};
pub use upstream_warmup::UpstreamWarmup;

#[cfg(test)]
pub(crate) use profile_resolver::tests::{Zone, exchange as zone_exchange};
