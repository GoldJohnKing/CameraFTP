// CameraFTP - A Cross-platform FTP companion for camera photo transfer
// Copyright (C) 2026 GoldJohnKing <GoldJohnKing@Live.cn>
// SPDX-License-Identifier: AGPL-3.0-or-later

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
fn private_lan_10_x_detected() {
    let ip: std::net::IpAddr = "10.0.0.5".parse().unwrap();
    assert!(NetworkManager::is_private_lan(&ip));
}

#[test]
fn private_lan_172_16_to_31_detected() {
    let ip_lo: std::net::IpAddr = "172.16.0.1".parse().unwrap();
    let ip_hi: std::net::IpAddr = "172.31.255.254".parse().unwrap();
    assert!(NetworkManager::is_private_lan(&ip_lo));
    assert!(NetworkManager::is_private_lan(&ip_hi));
}

#[test]
fn private_lan_172_outside_range_not_detected() {
    let ip: std::net::IpAddr = "172.32.0.1".parse().unwrap();
    assert!(!NetworkManager::is_private_lan(&ip));
    let ip2: std::net::IpAddr = "172.15.0.1".parse().unwrap();
    assert!(!NetworkManager::is_private_lan(&ip2));
}

#[test]
fn private_lan_192_168_detected() {
    let ip: std::net::IpAddr = "192.168.1.100".parse().unwrap();
    assert!(NetworkManager::is_private_lan(&ip));
}

#[test]
fn tun_fake_ip_not_private_lan() {
    // TUN 代理常见的 fake-IP 段，不应被识别为私网
    let ip: std::net::IpAddr = "198.18.0.1".parse().unwrap();
    assert!(!NetworkManager::is_private_lan(&ip));
}

#[test]
fn public_ip_not_private_lan() {
    let ip: std::net::IpAddr = "8.8.8.8".parse().unwrap();
    assert!(!NetworkManager::is_private_lan(&ip));
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
fn mihomo_detected_as_virtual() {
    assert!(NetworkManager::is_virtual_interface("Mihomo"));
}

#[test]
fn clash_detected_as_virtual() {
    assert!(NetworkManager::is_virtual_interface("Clash"));
}

#[test]
fn tailscale_detected_as_virtual() {
    assert!(NetworkManager::is_virtual_interface("tailscale0"));
}

#[test]
fn zerotier_detected_as_virtual() {
    assert!(NetworkManager::is_virtual_interface("ZeroTier [zcqrh5"));
}

#[test]
fn wireguard_detected_as_virtual() {
    assert!(NetworkManager::is_virtual_interface("WireGuard Tunnel"));
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

#[test]
fn iftype_loopback_is_virtual() {
    // IF_TYPE_SOFTWARE_LOOPBACK = 24
    assert!(NetworkManager::is_virtual_by_iftype(24));
}

#[test]
fn iftype_prop_virtual_is_virtual() {
    // IF_TYPE_PROP_VIRTUAL = 53
    // WireGuard wintun, OpenVPN TAP, Clash/Mihomo TUN 等用户态虚拟网卡
    assert!(NetworkManager::is_virtual_by_iftype(53));
}

#[test]
fn iftype_tunnel_is_virtual() {
    // IF_TYPE_TUNNEL = 131
    // OS 内核隧道：Teredo/6to4/ISATAP
    assert!(NetworkManager::is_virtual_by_iftype(131));
}

#[test]
fn iftype_ethernet_not_virtual() {
    // IF_TYPE_ETHERNET_CSMACD = 6
    assert!(!NetworkManager::is_virtual_by_iftype(6));
}

#[test]
fn iftype_wifi_not_virtual() {
    // IF_TYPE_IEEE80211 = 71
    assert!(!NetworkManager::is_virtual_by_iftype(71));
}

#[test]
fn iftype_other_not_virtual() {
    // IF_TYPE_OTHER = 1
    assert!(!NetworkManager::is_virtual_by_iftype(1));
}
