use std::fs;
use std::sync::Arc;
use std::time::SystemTime;
use rustls::{Certificate, Error, ServerName};
use rustls::client::ServerCertVerified;

pub struct SkipServerVerification;

impl SkipServerVerification {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

impl rustls::client::ServerCertVerifier for SkipServerVerification {
    fn verify_server_cert(&self, _end_entity: &Certificate, _intermediates: &[Certificate], _server_name: &ServerName, _scts: &mut dyn Iterator<Item=&[u8]>, _ocsp_response: &[u8], _now: SystemTime) -> Result<ServerCertVerified, Error> {
        Ok(rustls::client::ServerCertVerified::assertion())
    }
}

pub fn generate_self_signed_cert() -> Result<(rustls::Certificate, rustls::PrivateKey), Box<dyn std::error::Error>> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])?;
    let key = rustls::PrivateKey(cert.serialize_private_key_der());
    Ok((rustls::Certificate(cert.serialize_der()?), key))
}


pub fn read_cert_from_file() -> Result<(rustls::Certificate, rustls::PrivateKey), Box<dyn std::error::Error>> {
    // Read from certificate and key from directory.
    let (cert, key) = fs::read(&"./cert.pem").and_then(|x| Ok((x, fs::read(&"./privkey.pem")?)))?;

    // Parse to certificate chain whereafter taking the first certifcater in this chain.
    let cert = rustls::Certificate(cert);
    let key = rustls::PrivateKey(key);

    Ok((cert, key))
}

pub fn write_cert_to_file(certificate: rustls::Certificate, key: rustls::PrivateKey) -> Result<(), Box<dyn std::error::Error>> {
    let cert_data = certificate.0;
    let key_data = key.0;
    fs::write("./cert.pem", cert_data)?;
    fs::write("./privkey.pem", key_data)?;
    Ok(())
}

