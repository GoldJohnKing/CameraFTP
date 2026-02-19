use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use tokio::net::TcpListener;

#[derive(Debug, Clone, serde::Serialize)]
pub struct NetworkInterface {
    pub name: String,
    pub ip: String,
    pub is_wifi: bool,
    pub is_ethernet: bool,
    pub is_up: bool,
}

pub struct NetworkManager;

impl NetworkManager {
    /// Get all IPv4 addresses on this machine
    pub fn list_interfaces() -> Vec<NetworkInterface> {
        let mut interfaces = Vec::new();
        
        // Get local addresses using ifaddrs
        #[cfg(unix)]
        {
            use std::ffi::CStr;
            
            unsafe {
                let mut ifap: *mut libc::ifaddrs = std::ptr::null_mut();
                if libc::getifaddrs(&mut ifap) == 0 {
                    let mut ifa = ifap;
                    while !ifa.is_null() {
                        let ifa_ref = &*ifa;
                        if !ifa_ref.ifa_addr.is_null() {
                            let addr = &*(ifa_ref.ifa_addr as *const libc::sockaddr_in);
                            if addr.sin_family as i32 == libc::AF_INET {
                                let ip_bytes = addr.sin_addr.s_addr.to_be_bytes();
                                let ip = IpAddr::V4(Ipv4Addr::new(ip_bytes[0], ip_bytes[1], ip_bytes[2], ip_bytes[3]));
                                
                                // Skip loopback
                                if !ip.is_loopback() {
                                    let name = CStr::from_ptr(ifa_ref.ifa_name)
                                        .to_string_lossy()
                                        .to_string();
                                    
                                    let is_wifi = name.to_lowercase().contains("wlan") 
                                        || name.to_lowercase().contains("wi-fi")
                                        || name.to_lowercase().contains("wifi");
                                    let is_ethernet = name.to_lowercase().contains("eth")
                                        || name.to_lowercase().contains("en");
                                    
                                    interfaces.push(NetworkInterface {
                                        name,
                                        ip: ip.to_string(),
                                        is_wifi,
                                        is_ethernet,
                                        is_up: true,
                                    });
                                }
                            }
                        }
                        ifa = ifa_ref.ifa_next;
                    }
                    libc::freeifaddrs(ifap);
                }
            }
        }
        
        #[cfg(windows)]
        {
            // On Windows, use a simple approach
            // Try to get local IP by connecting to a public address
            if let Ok(socket) = std::net::UdpSocket::bind("0.0.0.0:0") {
                if socket.connect("8.8.8.8:80").is_ok() {
                    if let Ok(local_addr) = socket.local_addr() {
                        if let IpAddr::V4(ip) = local_addr.ip() {
                            interfaces.push(NetworkInterface {
                                name: "primary".to_string(),
                                ip: ip.to_string(),
                                is_wifi: false,
                                is_ethernet: true,
                                is_up: true,
                            });
                        }
                    }
                }
            }
        }
        
        interfaces
    }
    
    /// Recommend the best IP address
    /// Priority: WiFi > Ethernet > Others
    pub fn recommended_ip() -> Option<String> {
        let interfaces = Self::list_interfaces();
        
        // Prefer WiFi
        if let Some(iface) = interfaces.iter().find(|i| i.is_wifi) {
            return Some(iface.ip.clone());
        }
        
        // Then Ethernet
        if let Some(iface) = interfaces.iter().find(|i| i.is_ethernet) {
            return Some(iface.ip.clone());
        }
        
        // Finally any available
        interfaces.first().map(|i| i.ip.clone())
    }
    
    /// Check if a port is available
    pub async fn is_port_available(port: u16) -> bool {
        match TcpListener::bind(format!("0.0.0.0:{}", port)).await {
            Ok(_) => true,
            Err(_) => false,
        }
    }
    
    /// Find an available port starting from the given port
    pub async fn find_available_port(start: u16) -> Option<u16> {
        for port in start..=65535 {
            if Self::is_port_available(port).await {
                return Some(port);
            }
        }
        None
    }
}