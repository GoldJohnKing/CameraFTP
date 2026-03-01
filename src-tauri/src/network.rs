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
        name_lower.contains("wlan") 
            || name_lower.contains("wi-fi")
            || name_lower.contains("wifi")
            || name_lower.contains("wl")
            || name_lower.contains("wireless")
    }

    /// 判断是否为以太网接口
    fn is_ethernet_interface(name: &str) -> bool {
        let name_lower = name.to_lowercase();
        (name_lower.contains("eth")
            || name_lower.contains("en")
            || name_lower.contains("ethernet")
            || name_lower.contains("lan"))
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
        
        // 优先 WiFi
        if let Some(iface) = interfaces.iter().find(|i| i.is_wifi) {
            tracing::info!("Selected WiFi interface: {} ({})", iface.name, iface.ip);
            return Some(iface.ip.clone());
        }
        
        // 其次以太网
        if let Some(iface) = interfaces.iter().find(|i| i.is_ethernet) {
            tracing::info!("Selected Ethernet interface: {} ({})", iface.name, iface.ip);
            return Some(iface.ip.clone());
        }
        
        // 最后选择任何可用接口（但排除链路本地和虚拟网卡后应该不会走到这里）
        if let Some(iface) = interfaces.first() {
            tracing::info!("Selected fallback interface: {} ({})", iface.name, iface.ip);
            return Some(iface.ip.clone());
        }
        
        None
    }
    
    /// 检查端口是否可用
    pub async fn is_port_available(port: u16) -> bool {
        let addr = format!("0.0.0.0:{}", port);
        matches!(TcpListener::bind(&addr).await, Ok(_))
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

    /// 检查PASV端口范围内的可用端口数量
    /// 返回 (可用端口数量, 总端口数量, 第一个可用端口)
    pub async fn check_pasv_port_range(start: u16, end: u16) -> (usize, usize, Option<u16>) {
        let total = if end >= start { (end - start + 1) as usize } else { 0 };
        let mut available = 0;
        let mut first_available = None;

        for port in start..=end {
            if Self::is_port_available(port).await {
                available += 1;
                if first_available.is_none() {
                    first_available = Some(port);
                }
            }
        }

        tracing::info!(
            "PASV port range {}-{}: {}/{} available, first available: {:?}",
            start, end, available, total, first_available
        );

        (available, total, first_available)
    }

    /// 查找一个可用的100端口PASV范围
    /// 从 start 开始搜索，返回 (range_start, range_end)
    /// 如果找不到合适的范围则返回 None
    pub async fn find_available_pasv_range(start: u16) -> Option<(u16, u16)> {
        const RANGE_SIZE: u16 = 100;
        const MIN_AVAILABLE: usize = 10; // 范围内至少需要有10个可用端口

        // 确保 end 不会溢出
        let max_range_start = 65535 - RANGE_SIZE + 1;
        let search_start = start.min(max_range_start);

        // 按100端口块搜索
        let mut range_start = (search_start / RANGE_SIZE) * RANGE_SIZE;
        if range_start < search_start {
            range_start += RANGE_SIZE;
        }

        while range_start <= max_range_start {
            let range_end = range_start + RANGE_SIZE - 1;
            let (available, _, first) = Self::check_pasv_port_range(range_start, range_end).await;

            if available >= MIN_AVAILABLE {
                tracing::info!(
                    "Found available PASV range: {}-{} ({} ports available, first: {:?})",
                    range_start, range_end, available, first
                );
                return Some((range_start, range_end));
            }

            // 移动到下一个100端口块
            range_start += RANGE_SIZE;
        }

        tracing::error!("No available 100-port PASV range found starting from {}", start);
        None
    }
}
