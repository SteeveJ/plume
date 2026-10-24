use reqwest::Client;
use std::time::Duration;

/// Client HTTP avec délais : sans eux, un réseau figé bloque la dictée indéfiniment.
pub fn client(timeout: Duration) -> Client {
    Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .timeout(timeout)
        .build()
        .unwrap_or_else(|_| Client::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_server_that_never_answers_times_out() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let started = std::time::Instant::now();
        let err = client(Duration::from_millis(300)).get(url).send().await.unwrap_err();
        assert!(err.is_timeout(), "{err}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
