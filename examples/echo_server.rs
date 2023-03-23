use tokio::sync::mpsc;
use tracing::{error, info};
use websocket::parser::StringParser;
use websocket::server::{ServerEvent, ServerHandle};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let address = "127.0.0.1:8080";
    let (tx, mut rx) = mpsc::channel(1);
    let server_hdl = ServerHandle::<StringParser>::new(address.to_string(), tx)
        .await
        .expect("Could not start server.");
    let listen_addr = server_hdl
        .get_listen_address()
        .await
        .expect("Problem getting listen address.");

    info!("Echo server listening on address {listen_addr}");

    loop {
        if let Some(server_event) = rx.recv().await {
            match server_event {
                ServerEvent::Close => break,
                ServerEvent::NewConnection(addr) => {
                    info!("New connection from {addr}.");
                }
                ServerEvent::ConnectionClose(addr) => {
                    info!("Connection {addr} closed.");
                }
                ServerEvent::ConnectionRequest(request) => {
                    info!(
                        "Got request {} from {}.",
                        request.request.get_request(),
                        request.from
                    );
                    if let Err(e) = request
                        .request
                        .complete(request.request.get_request().clone())
                        .await
                    {
                        error!("Problem completing request: {e}");
                    }
                }
                ServerEvent::ConnectionEvent(event) => {
                    info!("Got event {} from {}.", event.event, event.from);
                    server_hdl
                        .event(event.from, event.event.clone())
                        .await
                        .expect("Problem sending server event");
                }
            }
        } else {
            break;
        }
    }

    info!("Echo server shutting down.");
}
