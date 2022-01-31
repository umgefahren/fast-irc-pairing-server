use std::error::Error;
use std::sync::Arc;

use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_stream::StreamExt;

pub(crate) async fn listen(mut incoming: quinn::Incoming) -> Result<(Arc<Mutex<Vec<JoinHandle<()>>>>, JoinHandle<()>), Box<dyn Error>> {
    let  conn_handlers = Arc::new(Mutex::new(Vec::new()));
    let loop_conn_handler = conn_handlers.clone();
    let loop_handle = tokio::spawn(async move {
        loop {
            let connecting = incoming.next().await.unwrap();
            println!("Got connection with remote connection => {}", connecting.remote_address());
            let connection = connecting.await.unwrap().bi_streams.next().await.unwrap();
            let (mut send_stream, mut recv_stream) = connection.unwrap();
            println!("Got Bi");
            let chunk = recv_stream.read_chunk(usize::MAX, true).await.unwrap().unwrap();

            println!("Initial Data {}", String::from_utf8(chunk.bytes.to_vec()).unwrap());
            println!("Received string");

            let handler = tokio::spawn(async move {
                let mut incoming_data: Vec<u8> = Vec::with_capacity(1_000);
                loop {
                    recv_stream.read_buf(&mut incoming_data).await.unwrap();

                    let incoming_string = String::from_utf8_lossy(&*incoming_data).to_string();

                    println!("In => {}", incoming_string);
                    println!("Writing Data => {:?}", incoming_data);
                    let send_result = send_stream.write_all(&mut incoming_data).await;
                    println!("Wrote to the back channel");
                    incoming_data.clear();
                    match send_result {
                        Ok(_) => {}
                        Err(_) => {
                            println!("Out here");
                            break;
                        }
                    };
                }
            });
            loop_conn_handler.lock().await.push(handler);
        };
    });

    Ok((conn_handlers, loop_handle))

}