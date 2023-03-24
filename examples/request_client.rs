use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info};
use rrev::client;
use rrev::parser::StringParser;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let address = "127.0.0.1:8080";
    let url = url::Url::parse(format!("ws://{address}").as_str()).expect("Problem parsing url");
    let (tx, _rx) = mpsc::channel(1);
    let client_hdl = client::connect::<StringParser>(url, tx)
        .await
        .expect("Problem connecting to the server");

    info!("Sending request");
    let reply = client_hdl
        .request_timeout("test".to_string(), Duration::from_secs(5))
        .await;

    match reply {
        Ok(r) => {
            info!("Got reply {r}");
        }
        Err(e) => {
            error!("Problem getting reply {e}")
        }
    }
}
