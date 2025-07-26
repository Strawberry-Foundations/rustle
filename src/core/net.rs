pub fn get_valid_ips(hostname: &str) -> Vec<String> {
    use std::net::ToSocketAddrs;
    let mut ips = Vec::new();
    let addr_str = format!("{hostname}:49242");
    if let Ok(addrs) = addr_str.to_socket_addrs() {
        for addr in addrs {
            let ip = addr.ip();
            if is_private_ip(&ip) {
                ips.push(ip.to_string());
            }
        }
    }
    ips
}

pub fn is_private_ip(ip: &std::net::IpAddr) -> bool {
    if let std::net::IpAddr::V4(ipv4) = ip {
        let octets = ipv4.octets();
        // 10.x.x.x
        if octets[0] == 10 {
            return true;
        }
        // 192.168.x.x
        if octets[0] == 192 && octets[1] == 168 {
            return true;
        }
        // 172.16.x.x - 172.31.x.x
        if octets[0] == 172 && (16..=31).contains(&octets[1]) {
            return true;
        }
    }
    false
}
