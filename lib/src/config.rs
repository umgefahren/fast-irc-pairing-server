use std::net::SocketAddr;
use std::sync::Arc;
use crate::certificates::SkipServerVerification;

pub static SERVER_NAME: &str = "localhost";

pub fn client_addr() -> SocketAddr {
    "127.0.0.1:5000".parse::<SocketAddr>().unwrap()
}

pub fn server_addr() -> SocketAddr {
    "127.0.0.1:5001".parse::<SocketAddr>().unwrap()
}

pub fn crypto_server_config(certificate: rustls::Certificate, key: rustls::PrivateKey) -> Result<quinn::ServerConfig, Box<dyn std::error::Error>> {
    let server_config = quinn::ServerConfig::with_single_cert(vec![certificate], key)?;
    Ok(server_config)
}

pub fn crypto_client_config(certificate: rustls::Certificate, key: rustls::PrivateKey) -> Result<quinn::ClientConfig, Box<dyn std::error::Error>> {
    let crypto = rustls::ClientConfig::builder()
        .with_safe_defaults()
        .with_custom_certificate_verifier(SkipServerVerification::new())
        .with_single_cert(vec![certificate], key)?;

    Ok(quinn::ClientConfig::new(Arc::new(crypto)))
}
