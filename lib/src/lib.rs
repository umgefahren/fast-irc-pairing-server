use std::error::Error;
use quinn::Endpoint;
use crate::config::client_addr;

pub mod certificates;
pub mod client;
pub mod config;
pub mod messages;

pub struct Client {
    pub endpoint: Endpoint,
}

impl Client {
    pub fn new() -> Result<Self, Box<dyn Error>> {

        let endpoint = Endpoint::client(client_addr()).unwrap();
        Ok(
            Self {
                endpoint
            }
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
