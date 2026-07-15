//! Identity: the API-key store (keys.json), the email allowlist message, the pure loopback
//! check, and the Google/xAI OAuth handlers.

pub mod keystore;
pub mod oauth_google;

pub use keystore::KeyStore;

use std::net::IpAddr;

use crate::config::Config;

pub fn denial_message(cfg: &Config, email: &str) -> String {
    let who = if email.is_empty() { "no verified email" } else { email };
    format!(
        "This user email ({who}) is not allowed yet. Contact the admin: {}.",
        cfg.admin_contact()
    )
}

/// THE loopback trust check: loopback is trusted (remote needs a key). Treats an
/// IPv4-mapped IPv6 loopback (`::ffff:127.0.0.1`) as loopback too, so local clients on a
/// dual-stack socket aren't wrongly forced to authenticate. Pure + unit-testable.
pub fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_loopback(),
        IpAddr::V6(v6) => {
            v6.is_loopback() || v6.to_ipv4_mapped().is_some_and(|v4| v4.is_loopback())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn loopback_variants_are_loopback() {
        assert!(is_loopback(IpAddr::V4(Ipv4Addr::LOCALHOST)));
        assert!(is_loopback(IpAddr::V6(Ipv6Addr::LOCALHOST)));
        // IPv4-mapped IPv6 loopback ::ffff:127.0.0.1
        assert!(is_loopback(IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped())));
    }

    #[test]
    fn remote_is_not_loopback() {
        assert!(!is_loopback(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))));
        assert!(!is_loopback(IpAddr::V6(Ipv4Addr::new(203, 0, 113, 7).to_ipv6_mapped())));
    }
}
