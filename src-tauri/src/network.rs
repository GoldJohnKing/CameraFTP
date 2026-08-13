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
    is_private_lan: bool,
    /// Windows IfType（来自 GetAdaptersAddresses），非 Windows 平台为 None
    if_type: Option<u32>,
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

    /// 检查 IP 是否为 RFC1918 私网地址 (局域网网段)
    /// TUN 代理的 fake-IP（如 198.18.x.x）不在私网范围内，借此可降权代理网卡
    ///
    /// 已知局限：部分 WireGuard/ZeroTier/Clash 配置会使用 RFC1918 网段
    /// （如 10.x）作为隧道地址，此时此函数无法区分它们与真实局域网，
    /// 需依赖 is_virtual_interface（名称关键字）或 is_virtual_by_iftype（Windows IfType）兜底。
    fn is_private_lan(ip: &std::net::IpAddr) -> bool {
        if let std::net::IpAddr::V4(ipv4) = ip {
            let o = ipv4.octets();
            // 10.0.0.0/8
            o[0] == 10
            // 172.16.0.0/12 (172.16.0.0 – 172.31.255.255)
            || (o[0] == 172 && (16..=31).contains(&o[1]))
            // 192.168.0.0/16
            || (o[0] == 192 && o[1] == 168)
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
            // 代理/组网工具的 TUN 虚拟网卡，名称可能不含 tun 关键字
            "mihomo", "clash", "tailscale", "zerotier", "wireguard",
        ];
        
        virtual_keywords.iter().any(|keyword| name_lower.contains(keyword))
    }

    /// 根据 Windows IfType 判断是否为虚拟/隧道网卡
    /// 仅用于 Windows 端，IfType 来自 GetAdaptersAddresses。
    ///
    /// 使用局部命名常量（数值与 windows crate 的 IF_TYPE_* 一致）
    /// 而非直接导入 windows crate 常量，以便跨平台单元测试。
    ///
    /// IF_TYPE_PPP (23) 有意排除：PPPoE 宽带用户可能仅通过 PPP 接口上网，
    /// 过滤它会导致这些用户丢失唯一可用 IP。PPTP/L2TP VPN 由名称关键字
    /// "vpn" 兜底。
    fn is_virtual_by_iftype(if_type: u32) -> bool {
        // 数值对应 windows::Win32::NetworkManagement::IpHelper 中的常量：
        // - IF_TYPE_SOFTWARE_LOOPBACK = 24（回环）
        // - IF_TYPE_PROP_VIRTUAL = 53（WireGuard wintun、OpenVPN TAP、Clash/Mihomo TUN 等用户态虚拟网卡）
        // - IF_TYPE_TUNNEL = 131（OS 内核隧道：Teredo/6to4/ISATAP）
        const VIRTUAL_IF_TYPES: &[u32] = &[24, 53, 131];
        VIRTUAL_IF_TYPES.contains(&if_type)
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

    /// On Windows, get a map of UP interfaces' FriendlyName → IfType.
    /// Uses GetAdaptersAddresses directly because local_ip_address crate doesn't check OperStatus.
    /// IfType is consumed for reliable virtual/tunnel detection (see is_virtual_by_iftype).
    #[cfg(target_os = "windows")]
    fn get_active_adapter_info() -> std::collections::HashMap<String, u32> {
        use std::collections::HashMap;
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
            return HashMap::new();
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
            return HashMap::new();
        }

        let mut adapter_info = HashMap::new();
        let mut current = adapter_ptr;

        while !current.is_null() {
            unsafe {
                let adapter = &*current;
                if adapter.OperStatus == IfOperStatusUp && !adapter.FriendlyName.is_null() {
                    // Mirror local_ip_address crate's from_utf16_lossy decoding so
                    // the join key is identical for all inputs (strict to_string()
                    // would return Err for unpaired surrogates and drop the adapter).
                    let friendly_name = String::from_utf16_lossy(adapter.FriendlyName.as_wide());
                    if !friendly_name.is_empty() {
                        adapter_info.insert(friendly_name, adapter.IfType);
                    }
                }
                current = adapter.Next;
            }
        }

        adapter_info
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
                    let is_private_lan = Self::is_private_lan(&ip);
                    
                    tracing::info!(
                        "Found network interface: {} = {} (wifi={}, ethernet={}, private_lan={})",
                        name, ip, is_wifi, is_ethernet, is_private_lan
                    );
                    
                    interfaces.push(NetworkInterface {
                        name,
                        ip: ip.to_string(),
                        is_wifi,
                        is_ethernet,
                        is_private_lan,
                        if_type: None,
                    });
                }
            }
            Err(e) => {
                tracing::warn!("Failed to list network interfaces: {}", e);
            }
        }

        // On Windows, filter out disconnected adapters (OperStatus not Up)
        // and virtual adapters by IfType (more reliable than name keywords).
        #[cfg(target_os = "windows")]
        {
            let adapter_info = Self::get_active_adapter_info();
            if !adapter_info.is_empty() {
                let before = interfaces.len();
                interfaces = interfaces
                    .into_iter()
                    .filter_map(|mut iface| {
                        match adapter_info.get(&iface.name) {
                            None => {
                                tracing::debug!(
                                    "Skipping disconnected interface: {} ({})",
                                    iface.name, iface.ip
                                );
                                None
                            }
                            Some(&if_type) => {
                                iface.if_type = Some(if_type);
                                if Self::is_virtual_by_iftype(if_type) {
                                    tracing::debug!(
                                        "Skipping virtual interface by IfType: {} ({}, IfType={})",
                                        iface.name, iface.ip, if_type
                                    );
                                    None
                                } else {
                                    tracing::debug!(
                                        "Kept interface: {} ({}, IfType={})",
                                        iface.name, iface.ip, if_type
                                    );
                                    Some(iface)
                                }
                            }
                        }
                    })
                    .collect();
                tracing::info!(
                    "Filtered {} interfaces to {} active non-virtual ones",
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
                    let is_private_lan = Self::is_private_lan(&ip);
                    interfaces.push(NetworkInterface {
                        name: "primary".to_string(),
                        ip: ip.to_string(),
                        is_wifi: false,
                        is_ethernet: true,
                        is_private_lan,
                        if_type: None,
                    });
                }
            }
        }
        
        interfaces
    }
    
    /// 推荐最佳 IP 地址
    /// 优先级（避免 TUN 代理 fake-IP 误选）：
    /// 1. 私网地址的 WiFi 接口
    /// 2. 私网地址的以太网接口
    /// 3. 任意私网地址接口
    /// 4. WiFi 接口（兜底）
    /// 5. 以太网接口（兜底）
    /// 6. 第一个可用接口（兜底）
    pub fn recommended_ip() -> Option<String> {
        let interfaces = Self::list_interfaces();

        if interfaces.is_empty() {
            tracing::error!("No valid network interface found");
            return None;
        }

        interfaces
            .iter()
            .find(|i| i.is_wifi && i.is_private_lan)
            .or_else(|| interfaces.iter().find(|i| i.is_ethernet && i.is_private_lan))
            .or_else(|| interfaces.iter().find(|i| i.is_private_lan))
            .or_else(|| interfaces.iter().find(|i| i.is_wifi))
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
mod tests;
