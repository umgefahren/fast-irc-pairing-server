use std::collections::HashMap;
use std::error::Error;
use std::hash::BuildHasherDefault;
use std::sync::Arc;
use chashmap::CHashMap;
use quinn::{NewConnection, RecvStream, SendStream};
use tokio::io::AsyncReadExt;
use tokio::task::JoinHandle;
use tokio::sync::oneshot::Sender;
use tokio_stream::StreamExt;
use lib::messages::{ClientRequest, ClientRequestKind, InitConnClientReq};
use thiserror::Error;
use flume::Sender as FSender;
use rustc_hash::{FxHasher, FxHashMap};

#[derive(Debug)]
pub(crate) struct InitBridgeRequest {
    pub peer_id: u64,
    pub conn_id: u64,
    pub send_stream: SendStream,
    pub back_sender: Sender<Result<InitConnResponse, Box<dyn Error + Send>>>,
}

#[derive(Debug)]
pub(crate) struct InitConnResponse {
    pub send_stream: SendStream,
}

pub(crate) struct BridgeHandler {
    process: JoinHandle<()>,
}

impl BridgeHandler {
    pub async fn new(sender: SendStream, receiver: RecvStream, req: InitBridgeRequest) -> Result<Self, Box<dyn Error + Send>> {
        let process = tokio::spawn(async move {
            let internal_sender = req.send_stream;
            let mut buf_writer = tokio::io::BufWriter::new(internal_sender);
            let mut buf_reader = tokio::io::BufReader::new(receiver);
            match tokio::io::copy_buf(&mut buf_reader, &mut buf_writer).await {
                Ok(_) => {},
                Err(_) => {},
            };
        });
        let response = InitConnResponse {
            send_stream: sender
        };
        req.back_sender.send(Ok(response)).unwrap();
        Ok(Self {
            process,
        })
    }

    pub async fn new_given(sender: SendStream, receiver: RecvStream) -> Result<Self, Box<dyn Error + Send>> {
        let process = tokio::spawn(async move {
            let mut buf_writer = tokio::io::BufWriter::new(sender);
            let mut buf_reader = tokio::io::BufReader::new(receiver);
            match tokio::io::copy_buf(&mut buf_reader, &mut buf_writer).await {
                Ok(_) => {},
                Err(_) => {},
            };
        });
        Ok(Self {
            process
        })
    }
}

pub(crate) struct InitBridgeRequestHandler {
    handle: JoinHandle<()>,
    sender: FSender<InitBridgeMessage>,
}

impl InitBridgeRequestHandler {
    pub async fn new() -> Self {
        let (sender, receiver) = flume::unbounded::<InitBridgeMessage>();
        let handle = tokio::spawn(async move {
            let mut state: HashMap<(u64, u64), InitBridgeRequest, BuildHasherDefault<FxHasher>> = FxHashMap::default();
           loop {
               let init_bridge_message = receiver.recv_async().await.unwrap();
               match init_bridge_message {
                   InitBridgeMessage::Get { conn_id, peer_id, sender } => {
                       let get_option = state.remove(&(conn_id, peer_id));
                       match get_option {
                           Some(d) => {
                               sender.send(Ok(d)).unwrap();
                           },
                           None => {
                               sender.send(Err(InitBridgeMessageError::NotExistent)).unwrap();
                           }
                       }
                   }
                   InitBridgeMessage::Set { req, peer_id, conn_id, sender } => {
                       let get_option = state.contains_key(&(conn_id, peer_id));
                       if get_option {
                           sender.send(Err(InitBridgeMessageError::AlreadyExists)).unwrap();
                       } else {
                           state.insert((conn_id, peer_id), req);
                           sender.send(Ok(())).unwrap();
                       }
                   }
               }
           };
        });
        Self {
            sender,
            handle
        }
    }

    pub async fn insert(&self, req: InitBridgeRequest, conn_id: u64, peer_id: u64) -> Result<(), InitBridgeMessageError> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let init_bridge_message = InitBridgeMessage::Set {
            req,
            peer_id,
            conn_id,
            sender
        };
        self.sender.send_async(init_bridge_message).await.unwrap();
        let response = receiver.await.unwrap()?;
        Ok(response)
    }

    pub async fn get(&self, conn_id: u64, peer_id: u64) -> Result<InitBridgeRequest, InitBridgeMessageError> {
        let (sender, receiver) = tokio::sync::oneshot::channel();
        let init_bridge_message = InitBridgeMessage::Get {
            conn_id,
            peer_id,
            sender
        };
        self.sender.send_async(init_bridge_message).await.unwrap();
        receiver.await.unwrap()
    }
}

pub(crate) enum InitBridgeMessage {
    Get {
        conn_id: u64,
        peer_id: u64,
        sender: Sender<Result<InitBridgeRequest, InitBridgeMessageError>>,
    },
    Set {
        conn_id: u64,
        peer_id: u64,
        req: InitBridgeRequest,
        sender: Sender<Result<(), InitBridgeMessageError>>,
    }
}

#[derive(Error, Debug)]
pub(crate) enum InitBridgeMessageError {
    #[error("Data is not existent")]
    NotExistent,
    #[error("Entry already exists")]
    AlreadyExists,
}

impl From<InitBridgeMessageError> for Box<dyn Error + Send> {
    fn from(e: InitBridgeMessageError) -> Self {
        Box::new(e)
    }
}


pub(crate) struct ConnHandler {
    //  Bridge handler keyed by conn_id and peer_id
    bridges: Arc<CHashMap<(u64, u64), BridgeHandler>>,
    handle: JoinHandle<Result<(), Box<dyn Error + Send>>>,
    bi_stream_handle: JoinHandle<Result<(), Box<dyn Error + Send>>>,
}

impl ConnHandler {
    pub async fn new(conn: NewConnection) -> Result<(Self, u64), Box<dyn Error + Send>> {
        let mut bi_stream = conn.bi_streams;
        let (mut manager_sender, mut manager_receiver) = bi_stream.next().await.unwrap().unwrap();
        let id = manager_receiver.read_u64().await.unwrap();
        let bridges = Arc::new(CHashMap::new());
        let (sender, receiver) = flume::unbounded::<InitBridgeRequest>();
        // let mut pending_bridge: Arc<CHashMap<(u64, u64), InitBridgeRequest>> = Arc::new(CHashMap::new());
        let pending_bridge = Arc::new(InitBridgeRequestHandler::new().await);
        let internal_pending_bridge = pending_bridge.clone();
        let handle = tokio::spawn(async move {
            loop {
                let init_bridge_request = receiver.recv_async().await.unwrap();
                let (peer_id, conn_id) = (init_bridge_request.peer_id, init_bridge_request.conn_id);
                internal_pending_bridge.insert(init_bridge_request, conn_id, peer_id).await?;
                let init_client_conn_request = InitConnClientReq::new(conn_id, peer_id);
                init_client_conn_request.to_stream(&mut manager_sender).await?;
            };
        });

        let internal_pending_bridge_bi_stream = pending_bridge.clone();
        let bi_stream_bridges = bridges.clone();
        let bi_stream_handle: JoinHandle<Result<(), Box<dyn Error + Send>>> = tokio::spawn(async move {
            loop {
                let e = bi_stream.next().await.unwrap();
                let (send_conn, mut recv_conn) = e.unwrap();
                let id_int = recv_conn.read_u8().await.unwrap();
                match id_int {
                    // Responding on invite
                    2 => {
                        let conn_id_result = recv_conn.read_u64().await;
                        let conn_id = match conn_id_result {
                            Ok(d) => d,
                            Err(_) => {
                                continue;
                            }
                        };
                        let peer_id_result = recv_conn.read_u64().await;
                        let peer_id = match peer_id_result {
                            Ok(d) => d,
                            Err(_) => {
                                continue;
                            }
                        };
                        // directly unwrapping when not present in bridge. A suspend should be implemented here.
                        let req =  internal_pending_bridge_bi_stream.get(conn_id, peer_id).await?;
                        let bridge = BridgeHandler::new(send_conn, recv_conn, req).await?;
                        bi_stream_bridges.insert((conn_id, peer_id), bridge);
                    },
                    // Let Client Request in
                    1 => {
                        let client_request = ClientRequest::from_stream(&mut recv_conn).await?;
                        let peer_id = client_request.id;
                        match client_request.kind {
                            ClientRequestKind::Connect(conn_id) => {
                                let (back_sender, back_receiver) = tokio::sync::oneshot::channel();
                                let init_bridge_request = InitBridgeRequest {
                                    peer_id,
                                    conn_id,
                                    send_stream: send_conn,
                                    back_sender
                                };
                                sender.send(init_bridge_request).unwrap();
                                let init_conn_response = back_receiver.await.unwrap()?;
                                let bridge = BridgeHandler::new_given(init_conn_response.send_stream, recv_conn).await?;
                                bi_stream_bridges.insert((conn_id, peer_id), bridge);
                            }
                            ClientRequestKind::DisconnectFrom(_) => {
                                unimplemented!()
                            }
                        }
                    },
                    _ => {
                        continue;
                    }
                }

            }
        });

        let ret = Self {
            bridges,
            handle,
            bi_stream_handle
        };
        Ok((ret, id))
    }
}

pub(crate) struct ConnHandlerManager {
    map: Arc<CHashMap<u64, ConnHandler>>,
    pub(crate) handle: JoinHandle<Result<(), Box<dyn Error + Send>>>,
}

impl ConnHandlerManager {
   pub async fn new(mut incoming: quinn::Incoming) -> Result<Self, Box<dyn Error + Send>> {
       let map = Arc::new(CHashMap::new());
       let internal_map = map.clone();
       let handle = tokio::spawn(async move {
           loop {
               let connecting = incoming.next().await.unwrap();
               println!("Got connection with remote connection => {}", connecting.remote_address());
               let connection = connecting.await.unwrap();
               let conn_handler = ConnHandler::new(connection).await?;
               internal_map.insert(conn_handler.1, conn_handler.0);
           };
       });
       Ok(Self {
           map,
           handle
       })
   }
    pub async fn cancel(&mut self) -> Result<(), Box<dyn Error + Send>> {
        self.handle.abort();
        Ok(())
    }
}
