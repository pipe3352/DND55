//! Optional automatic port forwarding through UPnP-IGD (many home routers support it; some disable
//! it). This is a convenience for direct hosting — the relay works without it.

use std::net::{IpAddr, SocketAddrV4};
use std::time::Duration;

/// Asks the router to forward TCP `port` to this computer for 4 hours. Returns the external address.
/// Blocking (up to a few seconds): call from a background thread.
pub fn map_port(port: u16) -> Result<std::net::SocketAddr, String> {
    let local_ip = match crate::invite::lan_address() {
        Some(IpAddr::V4(ip)) => ip,
        _ => return Err("no IPv4 LAN address found".into()),
    };
    let mut opts = igd_next::SearchOptions::default();
    opts.timeout = Some(Duration::from_secs(3));
    let gw = igd_next::search_gateway(opts).map_err(|e| format!("no UPnP router found: {e}"))?;
    let ext = gw.get_external_ip().map_err(|e| format!("router didn't report its address: {e}"))?;
    gw.add_port(igd_next::PortMappingProtocol::TCP, port, std::net::SocketAddr::V4(SocketAddrV4::new(local_ip, port)), 4 * 3600, "DND55")
        .map_err(|e| format!("router refused the port mapping: {e}"))?;
    Ok(std::net::SocketAddr::new(ext, port))
}
