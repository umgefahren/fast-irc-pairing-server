use std::error::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use lib::Client;
use lib::client::configure_client;
use lib::config::{server_addr, SERVER_NAME};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut client = Client::new()?;
    let client_config = configure_client();
    client.endpoint.set_default_client_config(client_config);
    let new_connection = client.endpoint.connect(server_addr(), SERVER_NAME).unwrap().await.unwrap();
    println!("Remote Address => {}", new_connection.connection.remote_address());
    let (mut send, mut recv) = new_connection.connection.open_bi().await?;
    let stdin = tokio::io::stdin();
    let buffer = BufReader::new(stdin);
    let mut lines = buffer.lines();

    // let mut read_buffer = Vec::new();
    send.write_all("Hello Server :)".as_bytes()).await?;
    println!("Connected to server");
    loop {
        let in_str = lines.next_line().await?.unwrap();
        let in_data = in_str.as_bytes();
        println!("Sending => {}", in_str);
        send.write_all(in_data).await?;
        println!("Wrote data");
        let chunk = recv.read_chunk(1_000_000, true).await?.unwrap();
        // recv.read_buf(&mut read_buffer).await?;
        let out_str = String::from_utf8(chunk.bytes.to_vec())?;
        println!("Receiving => {}", out_str);
    }
}
