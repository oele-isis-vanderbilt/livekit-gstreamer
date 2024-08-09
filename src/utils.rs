use rand::{distributions::Alphanumeric, thread_rng, Rng};

pub fn random_string(prefix: &str) -> String {
    let random_string: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect();
    format!("{}-{}", prefix, random_string)
}
