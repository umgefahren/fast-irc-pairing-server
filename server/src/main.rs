extern crate core;

use std::error::Error;
use crate::conn_handler_manager::ConnHandlerManager;
use crate::quic::gen_server;

mod quic;
mod client_acceptor;
mod conn_handler_manager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let read_cert_error = lib::certificates::read_cert_from_file();
    let (cert, key) = match read_cert_error {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", e);
            let (cert, key) = lib::certificates::generate_self_signed_cert()?;
            lib::certificates::write_cert_to_file(cert.clone(), key.clone())?;
            (cert, key)
        }
    };
    let (_endpoint, incoming) = gen_server(cert, key).await?;

    let conn_handler_manager = ConnHandlerManager::new(incoming).await.unwrap();
    conn_handler_manager.handle.await.unwrap();
    // listen(incoming).await?;
    Ok(())
}
