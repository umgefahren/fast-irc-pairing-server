use std::error::Error;
use bincode::{config, Encode, Decode, config::Configuration};
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConnError {
    #[error("Error in the Quic Layer")]
    QuicError,
    #[error("Unexpected request kind")]
    UnexpectedKind,
    #[error("Unknown error")]
    Unknown,
}

impl From<ConnError> for Box<dyn std::error::Error + Send> {
    fn from(e: ConnError) -> Self {
        return Box::new(e)
    }
}


#[derive(Encode, Decode, Eq, PartialEq, Debug, Clone, Copy)]
pub enum ClientRequestKind {
    Connect(u64),
    DisconnectFrom(u64),
}

impl ClientRequestKind {
    pub fn connect(id: u64) -> Self {
        Self::Connect(id)
    }

    pub fn disconnect(id: u64) -> Self {
        Self::DisconnectFrom(id)
    }
}

#[derive(Encode, Decode, PartialEq, Eq, Debug, Clone, Copy)]
pub struct ClientRequest {
    pub kind: ClientRequestKind,
    pub id: u64
}

impl ClientRequest {
    pub fn new(kind: ClientRequestKind, id: u64) -> Self {
        Self {
            kind,
            id,
        }
    }

pub async fn from_stream<T: AsyncReadExt + Unpin>(mut stream: T) -> Result<Self, ConnError> {
        let kind_uint_result = stream.read_u8().await;
        let kind_uint = match kind_uint_result {
            Ok(d) => d,
            Err(_) => {
                return Err(ConnError::QuicError);
            }
        };
        let kind_id_result = stream.read_u64().await;
        let kind_id = match kind_id_result {
            Ok(d) => d,
            Err(_) => {
                return Err(ConnError::QuicError);
            }
        };
        let kind = match kind_uint {
            0 => {
                ClientRequestKind::connect(kind_id)
            },
            1 => {
                ClientRequestKind::disconnect(kind_id)
            },
            _ => {
                return Err(ConnError::UnexpectedKind);
            }
        };
        let id_result = stream.read_u64().await;
        let id = match id_result {
            Ok(d) => d,
            Err(_) => {
                return Err(ConnError::QuicError);
            }
        };
        let ret = Self::new(kind, id);
        Ok(ret)
    }

    pub async fn to_stream<T: AsyncWriteExt + Unpin>(&self, mut stream: T) -> Result<(), Box<dyn Error>> {
        let (kind_uint, kind_id) = match self.kind {
            ClientRequestKind::Connect(d) => {(0, d)}
            ClientRequestKind::DisconnectFrom(d) => {(1, d)}
        };
        let id = self.id;
        stream.write_u8(kind_uint).await?;
        stream.write_u64(kind_id).await?;
        stream.write_u64(id).await?;
        Ok(())
    }
}

pub struct InitConnClientReq {
    pub conn_id: u64,
    pub id: u64,
}

impl InitConnClientReq {
    pub fn new(conn_id: u64, id: u64) -> Self {
        Self {
            conn_id,
            id
        }
    }

    pub async fn from_stream<T: AsyncReadExt + Unpin>(mut stream: T) -> Result<Self, Box<dyn Error>> {
        // Read the connection id from the stream
        let conn_id = stream.read_u64().await?;
        // Read the other-side-client id from the stream
        let id = stream.read_u64().await?;
        let ret = Self {
            conn_id,
            id,
        };
        Ok(ret)
    }

    pub async fn to_stream<T: AsyncWrite + AsyncWriteExt + Unpin>(&self, mut stream: T) -> Result<(), ConnError> {
        // Write the connection id to the stream
        let mut write_result = stream.write_u64(self.conn_id).await;
        match write_result {
            Ok(_) => {}
            Err(_) => {
                return Err(ConnError::QuicError)
            }
        }
        // Write the other-side-client id to the stream
        write_result = stream.write_u64(self.id).await;
        match write_result {
            Ok(_) => {}
            Err(_) => {
                return Err(ConnError::QuicError)
            }
        }
        Ok(())
    }
}

pub fn bincode_config() -> Configuration {
    config::standard()
}


#[cfg(test)]
pub mod messages_test {
    use tokio::net::{TcpListener, TcpStream};
    use crate::messages::{ClientRequest, ClientRequestKind};

    #[tokio::test]
    pub async fn stream_duplex() {
        let (client, server) = tokio::io::duplex(64);
        let request_kind = ClientRequestKind::connect(10);
        let request = ClientRequest::new(request_kind, 100);
        request.to_stream(client).await.unwrap();
        let ret_request = ClientRequest::from_stream(server).await.unwrap();
        assert_eq!(request, ret_request);
    }

    #[tokio::test]
    pub async fn stream_tcp() {
        let listener = TcpListener::bind("0.0.0.0:0").await.unwrap();
        let listener_address = listener.local_addr().unwrap();

        let handler = tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            let ret_request = ClientRequest::from_stream(socket).await.unwrap();
            return ret_request;
        });
        let request_kind = ClientRequestKind::connect(10);
        let request = ClientRequest::new(request_kind, 100);
        let stream = TcpStream::connect(listener_address).await.unwrap();
        request.to_stream(stream).await.unwrap();
        let ret_request = handler.await.unwrap();
        assert_eq!(request, ret_request);
    }
}