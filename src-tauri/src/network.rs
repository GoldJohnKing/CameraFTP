// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::net::SocketAddr;

use tokio::net::TcpListener;
use local_ip_address::{local_ip, list_afinet_netifas};

#[derive(Debug, Clone)]
struct NetworkInterface {
    name: String,
    ip: String,
    is_wifi: bool,
    is_ethernet: bool,
}

pub struct NetworkManager;

impl NetworkManager {
    /// 检查 IP 是否为链路本地地址 (169.254.0.0/16)
    fn is_link_local(ip: &std::net::IpAddr) -> bool {
        if let std::net::IpAddr::V4(ipv4) = ip {
            let octets = ipv4.octets();
            // 169.254.0.0/16
            octets[0] == 169 && octets[1] == 254
        } else {
            false
        }
    }

    /// 检查是否为虚拟网卡
    fn is_virtual_interface(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        let virtual_keywords = [
            "vmware", "vmnet", "virtualbox", "vbox",
            "tap", "tun", "vpn", "docker", "veth",
            "hyper-v", "hyperv", "wsl", "loopback",
            "pseudo", "teredo", "isatap",
        ];
        
        virtual_keywords.iter().any(|keyword| name_lower.contains(keyword))
    }

    /// 判断是否为 WiFi 接口
    fn is_wifi_interface(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        ["wlan", "wi-fi", "wifi", "wl", "wireless"].iter().any(|k| name_lower.contains(k))
    }

    /// 判断是否为以太网接口
    fn is_ethernet_interface(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        ["eth", "en", "ethernet", "lan"].iter().any(|k| name_lower.contains(k))
            && !Self::is_virtual_interface(name)
            && !Self::is_wifi_interface(name)
    }

    /// On Windows, get the set of interface FriendlyNames that are operationally UP.
    /// Uses GetAdaptersAddresses directly because local_ip_address crate doesn't check OperStatus.
    #[cfg(target_os = "windows")]
    fn get_active_interface_names() -> std::collections::HashSet<String> {
        use std::collections::HashSet;
        use windows::Win32::NetworkManagement::IpHelper::GetAdaptersAddresses;
        use windows::Win32::NetworkManagement::IpHelper::IP_ADAPTER_ADDRESSES_LH;
        use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;

        const AF_INET: u32 = 2;

        let mut size: u32 = 0;
        // First call: get required buffer size
        unsafe {
            let _ = GetAdaptersAddresses(AF_INET, Default::default(), None, None, &mut size);
        }

        if size == 0 {
            return HashSet::new();
        }

        // Allocate buffer aligned for IP_ADAPTER_ADDRESSES_LH
        let layout = std::alloc::Layout::new::<IP_ADAPTER_ADDRESSES_LH>();
        let aligned_size = ((size as usize) + layout.align() - 1) & !(layout.align() - 1);
        let mut buffer: Vec<u8> = vec![0u8; aligned_size];
        let adapter_ptr = buffer.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH;

        let result = unsafe {
            GetAdaptersAddresses(AF_INET, Default::default(), None, Some(adapter_ptr), &mut size)
        };

        if result != 0 {
            tracing::warn!("GetAdaptersAddresses failed with error code: {}", result);
            return HashSet::new();
        }

        let mut active_names = HashSet::new();
        let mut current = adapter_ptr;

        while !current.is_null() {
            unsafe {
                let adapter = &*current;
                if adapter.OperStatus == IfOperStatusUp && !adapter.FriendlyName.is_null() {
                    let friendly_name = adapter.FriendlyName
                        .to_string()
                        .unwrap_or_default();
                    if !friendly_name.is_empty() {
                        active_names.insert(friendly_name);
                    }
                }
                current = adapter.Next;
            }
        }

        active_names
    }

    /// 获取所有网络接口
    fn list_interfaces() -> Vec<NetworkInterface> {
        let mut interfaces = Vec::new();
        
        match list_afinet_netifas() {
            Ok(ifaces) => {
                for (name, ip) in ifaces {
                    // 只处理 IPv4 且非回环地址
                    if !ip.is_ipv4() || ip.is_loopback() {
                        continue;
                    }

                    // 排除链路本地地址 (169.254.x.x)
                    if Self::is_link_local(&ip) {
                        tracing::debug!("Skipping link-local address: {} on {}", ip, name);
                        continue;
                    }

                    // 排除虚拟网卡
                    if Self::is_virtual_interface(&name) {
                        tracing::debug!("Skipping virtual interface: {} ({})", name, ip);
                        continue;
                    }

                    let is_wifi = Self::is_wifi_interface(&name);
                    let is_ethernet = Self::is_ethernet_interface(&name);
                    
                    tracing::info!(
                        "Found network interface: {} = {} (wifi={}, ethernet={})",
                        name, ip, is_wifi, is_ethernet
                    );
                    
                    interfaces.push(NetworkInterface {
                        name,
                        ip: ip.to_string(),
                        is_wifi,
                        is_ethernet,
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Failed to list network interfaces: {}", e);
            }
        }

        // On Windows, filter out disconnected adapters whose OperStatus is not Up
        #[cfg(target_os = "windows")]
        {
            let active_names = Self::get_active_interface_names();
            if !active_names.is_empty() {
                let before = interfaces.len();
                interfaces.retain(|iface| active_names.contains(&iface.name));
                tracing::info!(
                    "Filtered {} interfaces to {} active ones",
                    before,
                    interfaces.len()
                );
            }
        }

        // 如果没有找到接口，尝试获取本地 IP（但确保不是链路本地地址）
        if interfaces.is_empty() {
            if let Ok(ip) = local_ip() {
                if ip.is_ipv4() 
                    && !ip.is_loopback() 
                    && !Self::is_link_local(&ip) 
                {
                    tracing::info!("Using fallback local IP: {}", ip);
                    interfaces.push(NetworkInterface {
                        name: "primary".to_string(),
                        ip: ip.to_string(),
                        is_wifi: false,
                        is_ethernet: true,
                    });
                }
            }
        }
        
        interfaces
    }
    
    /// 推荐最佳 IP 地址
    /// 优先级：WiFi > 以太网 > 其他
    pub fn recommended_ip() -> Option<String> {
        let interfaces = Self::list_interfaces();

        if interfaces.is_empty() {
            tracing::error!("No valid network interface found");
            return None;
        }

        interfaces
            .iter()
            .find(|i| i.is_wifi)
            .or_else(|| interfaces.iter().find(|i| i.is_ethernet))
            .or_else(|| interfaces.first())
            .map(|iface| {
                tracing::info!("Selected interface: {} ({})", iface.name, iface.ip);
                iface.ip.clone()
            })
    }
    
    /// 检查端口是否可用
    pub async fn is_port_available(port: u16) -> bool {
        let addr: SocketAddr = ([0, 0, 0, 0], port).into();
        TcpListener::bind(addr).await.is_ok()
    }
    
    /// 查找从起始端口开始的可用端口
    pub async fn find_available_port(start: u16) -> Option<u16> {
        for port in start..=65535 {
            if Self::is_port_available(port).await {
                tracing::info!("Found available port: {}", port);
                return Some(port);
            }
        }
        tracing::error!("No available port found in range {}-65535", start);
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_local_169_254_x_x_detected() {
        let ip: std::net::IpAddr = "169.254.1.1".parse().unwrap();
        assert!(NetworkManager::is_link_local(&ip));
    }

    #[test]
    fn normal_ip_not_link_local() {
        let ip: std::net::IpAddr = "192.168.1.1".parse().unwrap();
        assert!(!NetworkManager::is_link_local(&ip));
    }

    #[test]
    fn ipv6_not_link_local() {
        let ip: std::net::IpAddr = "::1".parse().unwrap();
        assert!(!NetworkManager::is_link_local(&ip));
    }

    #[test]
    fn loopback_not_link_local() {
        let ip: std::net::IpAddr = "127.0.0.1".parse().unwrap();
        assert!(!NetworkManager::is_link_local(&ip));
    }

    #[test]
    fn vmware_detected_as_virtual() {
        assert!(NetworkManager::is_virtual_interface("VMware Network Adapter"));
    }

    #[test]
    fn vmnet_detected_as_virtual() {
        assert!(NetworkManager::is_virtual_interface("vmnet8"));
    }

    #[test]
    fn virtualbox_detected_as_virtual() {
        assert!(NetworkManager::is_virtual_interface("VirtualBox Host-Only"));
    }

    #[test]
    fn docker_detected_as_virtual() {
        assert!(NetworkManager::is_virtual_interface("docker0"));
    }

    #[test]
    fn wsl_detected_as_virtual() {
        assert!(NetworkManager::is_virtual_interface("WSL (Hyper-V)"));
    }

    #[test]
    fn vpn_detected_as_virtual() {
        assert!(NetworkManager::is_virtual_interface("VPN Connection"));
    }

    #[test]
    fn tun_detected_as_virtual() {
        assert!(NetworkManager::is_virtual_interface("tun0"));
    }

    #[test]
    fn real_ethernet_not_virtual() {
        assert!(!NetworkManager::is_virtual_interface("Ethernet"));
    }

    #[test]
    fn real_wifi_not_virtual() {
        assert!(!NetworkManager::is_virtual_interface("Wi-Fi"));
    }

    #[test]
    fn virtual_detection_case_insensitive() {
        assert!(NetworkManager::is_virtual_interface("DOCKER"));
        assert!(NetworkManager::is_virtual_interface("Vmware"));
        assert!(NetworkManager::is_virtual_interface("Wsl"));
    }

    #[test]
    fn wlan_detected_as_wifi() {
        assert!(NetworkManager::is_wifi_interface("WLAN"));
    }

    #[test]
    fn wi_fi_detected_as_wifi() {
        assert!(NetworkManager::is_wifi_interface("Wi-Fi"));
    }

    #[test]
    fn wifi_detected_as_wifi() {
        assert!(NetworkManager::is_wifi_interface("wifi0"));
    }

    #[test]
    fn wl_prefix_detected_as_wifi() {
        assert!(NetworkManager::is_wifi_interface("wlp3s0"));
    }

    #[test]
    fn wireless_detected_as_wifi() {
        assert!(NetworkManager::is_wifi_interface("wireless0"));
    }

    #[test]
    fn ethernet_not_wifi() {
        assert!(!NetworkManager::is_wifi_interface("Ethernet"));
    }

    #[test]
    fn eth_prefix_detected_as_ethernet() {
        assert!(NetworkManager::is_ethernet_interface("eth0"));
    }

    #[test]
    fn en_prefix_detected_as_ethernet() {
        assert!(NetworkManager::is_ethernet_interface("en0"));
    }

    #[test]
    fn ethernet_keyword_detected() {
        assert!(NetworkManager::is_ethernet_interface("Ethernet"));
    }

    #[test]
    fn lan_keyword_detected() {
        assert!(NetworkManager::is_ethernet_interface("LAN Connection"));
    }

    #[test]
    fn virtual_ethernet_excluded() {
        assert!(!NetworkManager::is_ethernet_interface("veth123456"));
    }

    #[test]
    fn wifi_not_ethernet() {
        assert!(!NetworkManager::is_ethernet_interface("wlan0"));
    }

    #[test]
    fn recommended_ip_does_not_panic() {
        let _ = NetworkManager::recommended_ip();
    }
}
