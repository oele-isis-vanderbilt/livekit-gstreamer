pub fn host_name() -> String {
    gethostname::gethostname().to_string_lossy().to_string()
}

pub fn get_ip_address() -> Option<String> {
    local_ip_address::local_ip().ok().map(|ip| ip.to_string())
}
