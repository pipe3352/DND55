//! Invite strings: everything a player needs to join, in one copy-pasteable line.
//!
//! * Direct: `dnd55://direct/203.0.113.5:7878/ABC123`
//! * Relay:  `dnd55://relay/relay.example.com:7979/K7P2QX9M/ABC123`
//!
//! The last part is always the session code. Plain `host:port` plus a separate code is also accepted.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    Direct(String),
    Relay { relay: String, invite: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invite {
    pub target: Target,
    pub session_code: String,
}

impl Invite {
    pub fn direct(addr: &str, code: &str) -> Self {
        Invite { target: Target::Direct(addr.into()), session_code: code.into() }
    }
    pub fn relay(relay: &str, invite: &str, code: &str) -> Self {
        Invite { target: Target::Relay { relay: relay.into(), invite: invite.into() }, session_code: code.into() }
    }

    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        let rest = s.strip_prefix("dnd55://")?;
        let parts: Vec<&str> = rest.split('/').filter(|p| !p.is_empty()).collect();
        match parts.as_slice() {
            ["direct", addr, code] if valid_addr(addr) && valid_code(code) => Some(Invite::direct(addr, &code.to_uppercase())),
            ["relay", relay, invite, code] if valid_addr(relay) && valid_code(invite) && valid_code(code) => Some(Invite::relay(relay, &invite.to_uppercase(), &code.to_uppercase())),
            _ => None,
        }
    }
}

impl std::fmt::Display for Invite {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.target {
            Target::Direct(a) => write!(f, "dnd55://direct/{a}/{}", self.session_code),
            Target::Relay { relay, invite } => write!(f, "dnd55://relay/{relay}/{invite}/{}", self.session_code),
        }
    }
}

fn valid_code(c: &str) -> bool {
    (4..=16).contains(&c.len()) && c.chars().all(|ch| ch.is_ascii_alphanumeric())
}

fn valid_addr(a: &str) -> bool {
    a.len() <= 255 && a.rsplit_once(':').is_some_and(|(h, p)| !h.is_empty() && p.parse::<u16>().is_ok()) && !a.contains(char::is_whitespace)
}

/// Best-effort LAN address of this computer (no packets are sent).
pub fn lan_address() -> Option<std::net::IpAddr> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?; // TEST-NET-1: routing lookup only
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invite_roundtrip_and_validation() {
        let d = Invite::direct("192.168.1.20:7878", "ABC123");
        assert_eq!(d.to_string(), "dnd55://direct/192.168.1.20:7878/ABC123");
        assert_eq!(Invite::parse(&d.to_string()), Some(d));
        let r = Invite::relay("relay.example.com:7979", "K7P2QX9M", "ABC123");
        assert_eq!(Invite::parse(&format!("  {r}  ")), Some(r.clone()));
        assert_eq!(Invite::parse("dnd55://relay/relay.example.com:7979/k7p2qx9m/abc123"), Some(r));
        for bad in ["", "http://x", "dnd55://direct/nope/ABC123", "dnd55://relay/r:1/!!/ABC123", "dnd55://direct/a b:1/ABC123"] {
            assert!(Invite::parse(bad).is_none(), "{bad}");
        }
    }
}
