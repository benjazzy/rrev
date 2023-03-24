use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};
use rrev::client;
use rrev::client::ConnectionEvent;
use rrev::parser::StringParser;

async fn receive_events(mut rx: mpsc::Receiver<ConnectionEvent<StringParser>>) {
    info!("Listening for messages.");
    while let Some(event) = rx.recv().await {
        match event {
            ConnectionEvent::Close => {
                info!("Client connected");
            }
            ConnectionEvent::EventMessage(e) => {
                info!("Got event {e}");
            }
            ConnectionEvent::RequestMessage(r) => {
                info!("Got request {}", r.get_request());
                if let Err(e) = r.complete(r.get_request().clone()).await {
                    error!("Problem completing request {e}");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let address = "127.0.0.1:8080";
    let url = url::Url::parse(format!("ws://{address}").as_str()).expect("Problem parsing url");
    let (tx, rx) = mpsc::channel(1);
    let client_hdl = client::connect::<StringParser>(url, tx)
        .await
        .expect("Problem connecting to the server");

    tokio::spawn(receive_events(rx));

    let mut interval = tokio::time::interval(Duration::from_secs(5));
    let mut count = 0;
    loop {
        interval.tick().await;
        info!("Sending event \"count: {count}\"");
        if let Err(e) = client_hdl
            .event(format!("count: {count}").to_string())
            .await
        {
            error!("Problem sending event. Exiting: {e}");
            break;
        }
        count += 1;
    }
}
