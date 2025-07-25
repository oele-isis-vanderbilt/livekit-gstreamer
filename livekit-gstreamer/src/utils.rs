use rand::{distributions::Alphanumeric, thread_rng, Rng};

pub fn random_string(prefix: &str) -> String {
    let random_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();
    format!("{}-{}", prefix, random_string)
}

pub fn system_time_nanos() -> i64 {
    let now = std::time::SystemTime::now();
    now.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as i64)
        .unwrap_or(0)
}
