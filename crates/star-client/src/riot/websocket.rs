use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

#[derive(Debug, Clone)]
pub enum RiotEvent {
    PresenceUpdate(serde_json::Value),
    ChatMessage(serde_json::Value),
    Disconnected,
}

pub async fn connect_websocket(
    port: u16,
    password: &str,
    tx: mpsc::UnboundedSender<RiotEvent>,
) -> Result<()> {
    let url = super::endpoints::local_websocket(port, password);

    let connector = tokio_tungstenite::Connector::NativeTls(
        native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?,
    );

    let (ws_stream, _) =
        tokio_tungstenite::connect_async_tls_with_config(url, None, false, Some(connector)).await?;

    let (mut write, mut read) = ws_stream.split();

    // Subscribe to presence and chat events
    let subscribe_presence = serde_json::json!([5, "OnJsonApiEvent_chat_v4_presences"]).to_string();
    let subscribe_chat = serde_json::json!([5, "OnJsonApiEvent_chat_v6_messages"]).to_string();

    write.send(Message::Text(subscribe_presence)).await?;
    write.send(Message::Text(subscribe_chat)).await?;

    tracing::info!("WebSocket connected, listening for events");

    while let Some(msg) = read.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(text.as_ref()) {
                    if let Some(arr) = val.as_array() {
                        if arr.len() >= 3 {
                            let uri = arr[1].as_str().unwrap_or("");
                            let data = arr[2].clone();
                            if uri.contains("presences") {
                                let _ = tx.send(RiotEvent::PresenceUpdate(data));
                            } else if uri.contains("messages") {
                                let _ = tx.send(RiotEvent::ChatMessage(data));
                            }
                        }
                    }
                }
            }
            Ok(Message::Close(_)) => {
                tracing::info!("WebSocket closed by server");
                let _ = tx.send(RiotEvent::Disconnected);
                break;
            }
            Err(e) => {
                tracing::error!("WebSocket error: {}", e);
                let _ = tx.send(RiotEvent::Disconnected);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
