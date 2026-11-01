//! Only the dashboard's own page gets in: the Host must be one of this
//! server's names (no DNS rebinding), an Origin must be the page's own (no
//! other site or other port on this computer), a POST must be JSON and an
//! API call must carry `X-Maestro: 1`, neither of which another site can send
//! without a CORS preflight that nothing here answers.

use std::net::{Ipv4Addr, UdpSocket};

use tiny_http::Method;

/// The names this server answers to. Anything else is another site: a DNS
/// rebinding page (Host: evil.com) or a page on another port of this computer
/// (Origin: http://127.0.0.1:3000) that the browser would happily send our
/// cookie from.
#[derive(Clone)]
pub(crate) struct Guard {
    /// The page's port: Host of a page request, and the port in every Origin.
    pub(crate) port: u16,
    /// The terminal WebSocket's port (port + 1): Host of a socket handshake.
    pub(crate) ws_port: u16,
    /// The LAN address shown in Settings, when other devices are let in.
    pub(crate) lan_ip: Option<Ipv4Addr>,
    /// Other devices are let in: every address of this computer is ours,
    /// a VPN's (Tailscale's 100.x.y.z, from another network) included.
    pub(crate) others: bool,
}

impl Guard {
    fn names(&self) -> Vec<String> {
        let mut out = vec!["127.0.0.1".to_string(), "localhost".to_string()];
        if let Some(ip) = self.lan_ip {
            out.push(ip.to_string());
        }
        out
    }

    /// Is `host` (a Host header) one of ours on `port`?
    pub(crate) fn host_ok(&self, host: Option<&str>, port: u16) -> bool {
        let Some(host) = host.map(|h| h.trim().to_ascii_lowercase()) else { return false };
        if self.names().iter().any(|n| host == format!("{n}:{port}") || (port == 80 && host == *n)) {
            return true;
        }
        // An address typed straight into the browser can't be a rebinding
        // page's (that needs a name), so any address of this computer will do.
        let addr = host.strip_suffix(&format!(":{port}")).unwrap_or(if port == 80 { &host } else { "" });
        self.others && addr.parse::<Ipv4Addr>().is_ok_and(is_own_ip)
    }

    /// No Origin (not a browser, or a same-origin GET), or the page's own.
    pub(crate) fn origin_ok(&self, origin: Option<&str>) -> bool {
        let Some(origin) = origin.map(|o| o.trim().to_ascii_lowercase()) else { return true };
        let Some(host) = origin.strip_prefix("http://") else { return false };
        self.host_ok(Some(host), self.port)
    }

    /// Why an HTTP request is turned away before any route sees it.
    pub(crate) fn refuse(&self, method: &Method, path: &str, get: impl Fn(&str) -> Option<String>) -> Option<(u16, &'static str)> {
        if !self.host_ok(get("Host").as_deref(), self.port) {
            return Some((403, "unknown host"));
        }
        if !self.origin_ok(get("Origin").as_deref()) {
            return Some((403, "foreign origin"));
        }
        // JSON only: a cross-site form or no-cors fetch can't send it without
        // a preflight, and nothing here answers one.
        if *method == Method::Post {
            let ct = get("Content-Type").unwrap_or_default();
            let mime = ct.split(';').next().unwrap_or("").trim();
            if !mime.eq_ignore_ascii_case("application/json") {
                return Some((415, "send JSON"));
            }
        }
        // The page's own calls carry X-Maestro; another site's can't without a preflight.
        let api = path.starts_with("/api/") || path == "/term" || path.starts_with("/term/");
        if api && get("X-Maestro").as_deref().map(str::trim) != Some("1") {
            return Some((403, "not from the dashboard"));
        }
        None
    }
}

/// Best-effort primary LAN IPv4 (no packet is actually sent: the connect just
/// picks the outbound interface). None if it can't be determined.
pub(crate) fn local_ip() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("8.8.8.8:80").ok()?;
    match sock.local_addr().ok()?.ip() {
        std::net::IpAddr::V4(v4) => Some(v4),
        _ => None,
    }
}

/// Is `ip` an address of this computer, on any network it is on (a VPN's
/// included)? A socket can be bound to an address only if it is ours.
pub(crate) fn is_own_ip(ip: Ipv4Addr) -> bool {
    !ip.is_unspecified() && !ip.is_broadcast() && !ip.is_multicast() && UdpSocket::bind((ip, 0)).is_ok()
}

/// The URLs to show, and the LAN address (when let in) the server answers to.
pub(crate) fn urls_for(port: u16, lan: bool) -> (Vec<String>, Option<Ipv4Addr>) {
    let mut out = vec![format!("http://127.0.0.1:{port}")];
    let ip = if lan { local_ip() } else { None };
    if let Some(ip) = ip {
        out.push(format!("http://{ip}:{port}"));
    }
    (out, ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_guard_knows_its_own_names() {
        let g = Guard { port: 8477, ws_port: 8478, lan_ip: Some(Ipv4Addr::new(192, 168, 1, 7)), others: true };
        assert!(g.host_ok(Some("192.168.1.7:8477"), 8477));
        assert!(g.host_ok(Some("192.168.1.7:8478"), 8478));
        assert!(!g.host_ok(Some("192.168.1.7:8478"), 8477));
        assert!(!g.host_ok(Some("192.168.1.8:8477"), 8477));
        assert!(!g.host_ok(None, 8477));
        assert!(g.origin_ok(None));
        assert!(g.origin_ok(Some("http://192.168.1.7:8477")));
        assert!(!g.origin_ok(Some("http://192.168.1.7:8478")), "the socket port is not a page");
        assert!(!g.origin_ok(Some("http://192.168.1.7:8477/x")));
        let local = Guard { lan_ip: None, others: false, ..g };
        assert!(!local.host_ok(Some("192.168.1.7:8477"), 8477), "the LAN address only when let in");
    }

    /// From another network the phone comes in through a VPN such as
    /// Tailscale, at another address of this computer (100.x.y.z): let in,
    /// every address of this computer is ours; an address of no network it
    /// is on never is.
    #[test]
    fn let_in_every_address_of_this_computer_answers() {
        let g = Guard { port: 8477, ws_port: 8478, lan_ip: None, others: true };
        assert!(!g.host_ok(Some("203.0.113.9:8477"), 8477), "not an address of this computer");
        assert!(!g.host_ok(Some("0.0.0.0:8477"), 8477));
        assert!(!g.host_ok(Some("evil.example:8477"), 8477));
        if let Some(ip) = local_ip() {
            let host = format!("{ip}:8477");
            assert!(g.host_ok(Some(&host), 8477));
            assert!(g.origin_ok(Some(&format!("http://{host}"))));
            let local = Guard { others: false, ..g };
            assert!(!local.host_ok(Some(&host), 8477), "only when other devices are let in");
        }
    }
}
