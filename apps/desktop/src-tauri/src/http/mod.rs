use parking_lot::RwLock;
use reqwest::Client;
use std::sync::OnceLock;
use std::time::Duration;

static CLIENT: OnceLock<RwLock<Client>> = OnceLock::new();

pub fn client() -> Client {
    let cell = CLIENT.get_or_init(|| RwLock::new(build_client()));
    cell.read().clone()
}

fn build_client() -> Client {
    Client::builder()
        .timeout(Duration::from_secs(30))
        .gzip(true)
        .deflate(true)
        .redirect(reqwest::redirect::Policy::limited(20))
        .build()
        .expect("reqwest client must build with rustls")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_builds_without_panic() {
        let _ = build_client();
    }

    #[test]
    fn client_singleton_initialised_lazily() {
        assert!(client().get(crate::versions::SUPPORTED_URL).build().is_ok());
    }
}
