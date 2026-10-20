//! Made-up keys and `.conf` texts for tests. None of them is a real key.

use std::io;

use super::{Profile, SecretBytes, SecretStore, WgConfParser};

pub const PRIVATE: &str = "yAnz5TF+lXXJte14tji3zlMNq+hd2rYUIgJBgB3fBmk=";
pub const PUBLIC: &str = "xTIBA5rboUvnH4htodjb6e697QjLERt1NAB4mZqp8Dg=";
pub const PRESHARED: &str = "FpCyhws9cxwWoV4xELtfJvjJN+zQVRPISllRWgeopVE=";
pub const OTHER_PRIVATE: &str = "cGFydG5lci1wcml2YXRlLWtleS1tYWRlLXVwLTAwMDA=";

/// A `.conf` with every kind of line.
pub fn full_conf() -> String {
    format!(
        "[Interface]\n\
         PrivateKey = {PRIVATE}\n\
         Address = 10.88.4.12/24, fd86:ea04:1115::46/64\n\
         DNS = 10.66.0.53, 10.66.0.54, corp.example.com\n\
         MTU = 1380\n\
         ListenPort = 51820\n\
         Table = off\n\
         PostUp = route add 10.0.0.0 mask 255.0.0.0 10.88.4.1 # secret script\n\
         SaveConfig = true\n\
         Jc = 4\n\
         \n\
         # the company gateway\n\
         [Peer]\n\
         PublicKey = {PUBLIC}\n\
         PresharedKey = {PRESHARED}\n\
         AllowedIPs = 0.0.0.0/0, ::/0\n\
         Endpoint = vpn.example.com:51820\n\
         PersistentKeepalive = 25\n"
    )
}

/// A minimal `.conf` with its own address, DNS and endpoint.
pub fn conf(private_key: &str, address: &str, dns: &str, endpoint: &str) -> String {
    format!(
        "[Interface]\nPrivateKey = {private_key}\nAddress = {address}\nDNS = {dns}\n\
         [Peer]\nPublicKey = {PUBLIC}\nEndpoint = {endpoint}\n"
    )
}

/// The profile of a `.conf` with every value set, imported at a fixed time.
pub fn sample_profile() -> Profile {
    let conf = WgConfParser::parse(&format!(
        "[Interface]\nPrivateKey = {PRIVATE}\nAddress = 10.88.4.12/24\nDNS = 10.66.0.53, 10.66.0.54\n\
         MTU = 1380\n[Peer]\nPublicKey = {PUBLIC}\nPresharedKey = {PRESHARED}\n\
         Endpoint = vpn.example.com:51820\nPersistentKeepalive = 25\n"
    ))
    .into_conf()
    .expect("a valid fixture");
    Profile::from_conf(conf, 0x6b29fc40_ca47_4067_b31d_00dd010662da, 1_792_423_315)
}

/// A reversible stand-in for DPAPI: the blob is the plaintext with every byte inverted and a marker,
/// so a test can tell a blob from plaintext.
pub struct FakeStore;

impl FakeStore {
    pub const MARKER: &'static [u8] = b"FAKE";
}

impl SecretStore for FakeStore {
    fn protect(&self, plaintext: &SecretBytes) -> io::Result<Vec<u8>> {
        let mut blob = Self::MARKER.to_vec();
        blob.extend(plaintext.expose().iter().map(|b| !b));
        Ok(blob)
    }

    fn unprotect(&self, blob: &[u8]) -> io::Result<SecretBytes> {
        let body = blob
            .strip_prefix(Self::MARKER)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "not a fake blob"))?;
        Ok(SecretBytes::from_vec(body.iter().map(|b| !b).collect()))
    }
}
