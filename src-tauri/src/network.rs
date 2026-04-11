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
