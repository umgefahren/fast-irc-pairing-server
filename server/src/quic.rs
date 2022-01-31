use std::error::Error;
use quinn::{Endpoint, Incoming};
use lib::config::{crypto_server_config, server_addr};

pub(crate) async fn gen_server(certificate: rustls::Certificate, key: rustls::PrivateKey) -> Result<(Endpoint, Incoming), Box<dyn Error>> {
    let server_config = crypto_server_config(certificate, key)?;
    let (endpoint, incoming) = Endpoint::server(
        server_config,
        server_addr()
    )?;
    Ok((endpoint, incoming))
}