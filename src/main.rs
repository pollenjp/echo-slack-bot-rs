use anyhow::{Context as _, Result, bail};
use futures_util::{SinkExt, StreamExt as _};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::connect_async;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}

/// https://api.slack.com/methods/apps.connections.open
///
/// ```json
/// {
///   "ok": true,
///   "url": "wss://wss-somethiing.slack.com/link/?ticket=12348&app_id=5678"
/// }
/// ```
#[derive(Deserialize, Debug)]
pub struct SlackApiAppConnectionsOpenResponse {
    pub ok: bool,
    pub url: Option<String>,
    pub error: Option<String>,
}

pub async fn open_connections(token: &str) -> Result<SlackApiAppConnectionsOpenResponse> {
    let client = reqwest::Client::new();
    let response = client
        .post("https://slack.com/api/apps.connections.open")
        .header("Authorization", format!("Bearer {}", token))
        .send()
        .await?
        .json::<SlackApiAppConnectionsOpenResponse>()
        .await?;
    Ok(response)
}

struct SlackClient {
    token: String,
}

impl SlackClient {
    pub async fn send_message(&self, channel: &str, text: &str) -> Result<()> {
        let client = reqwest::Client::new();
        let response = client
            .post("https://slack.com/api/chat.postMessage")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json; charset=utf-8")
            .json(&serde_json::json!({
                "channel": channel,
                "text": text,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            bail!("Failed to send message: {}", response.status());
        }

        Ok(())
    }
}

/// https://api.slack.com/apis/socket-mode#events
///
/// ```json
/// {
///   "payload": <event_payload>,
///   "envelope_id": <unique_identifier_string>,
///   "type": <event_type_enum>,
///   "accepts_response_payload": <accepts_response_payload_bool>
/// }
/// ```
///
#[derive(Deserialize, Debug)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum SocketModeMessage<'s> {
    Hello {},
    Disconnect {
        reason: &'s str,
    },
    EventsApi {
        payload: serde_json::Value,
        envelope_id: &'s str,
    },
    SlashCommands {
        payload: serde_json::Value,
        envelope_id: &'s str,
    },
    Interactive {
        payload: serde_json::Value,
        envelope_id: &'s str,
    },
}

#[derive(Deserialize, Serialize, Debug)]
struct MentionedPayload {
    pub event: MentionedPayloadEvent,
}

#[derive(Deserialize, Serialize, Debug)]
struct MentionedPayloadEvent {
    pub channel: String,
    pub text: Option<String>,
}

#[derive(Serialize)]
pub struct SocketModeAcknowledgeMessage<'s> {
    pub envelope_id: &'s str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<&'s str>,
}

struct RawConfig {
    app_level_token: String,
    user_oauth_token: String,
}

impl RawConfig {
    pub fn from_env() -> Self {
        let app_level_token_key = "SLACK_APP_LEVEL_TOKEN";
        let user_oauth_token_key = "SLACK_USER_OAUTH_TOKEN";
        Self {
            app_level_token: std::env::var("SLACK_APP_LEVEL_TOKEN").expect(&format!(
                "Please set the environment variable {}",
                app_level_token_key
            )),
            user_oauth_token: std::env::var("SLACK_USER_OAUTH_TOKEN").expect(&format!(
                "Please set the environment variable {}",
                user_oauth_token_key
            )),
        }
    }
}

async fn run() -> Result<()> {
    let config = RawConfig::from_env();

    let slack_client = SlackClient {
        token: config.user_oauth_token,
    };

    let con_result = open_connections(&config.app_level_token)
        .await
        .with_context(|| "connecting to slack api")?;

    if !con_result.ok {
        bail!(
            "connecting to app.connections.open: {}",
            con_result.error.as_deref().unwrap_or("Unknown error")
        );
    }

    let wss_url = con_result
        .url
        .ok_or_else(|| anyhow::anyhow!("missing wss url from server"))?;

    let (stream, _) = connect_async(wss_url).await?;
    let (mut write, mut read) = stream.split();

    // let mut read_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(read, true, None);
    // let mut write_stream = tokio_tungstenite::WebSocketStream::from_raw_socket(write, true, None);

    while let Some(m) = read.next().await {
        let m = match m {
            Ok(m) => m,
            Err(e) => {
                println!("Failed to read websocket frame: {:?}", e);
                continue;
            }
        };

        // debug message
        println!("message {:?}", m);

        // https://api.slack.com/apis/socket-mode#events
        match m {
            tungstenite::Message::Ping(bytes) => {
                println!("ping: {:?}", bytes);
            }
            tungstenite::Message::Text(t) => match serde_json::from_str(&t) {
                Ok(SocketModeMessage::Hello { .. }) => {
                    println!("Hello: {}", t);
                }
                Ok(SocketModeMessage::Disconnect { reason, .. }) => {
                    println!("Disconnect request: {}", reason);
                    break;
                }
                Ok(SocketModeMessage::EventsApi {
                    payload,
                    envelope_id,
                    ..
                }) => {
                    println!("Received Events API Message: {:?}", payload);

                    // reply ack message
                    // https://api.slack.com/apis/socket-mode#acknowledge
                    //
                    // {
                    //   "envelope_id": <$unique_identifier_string>,
                    //   "payload": <$payload_shape> // optional
                    // }
                    //
                    let ack_message = serde_json::to_string(&SocketModeAcknowledgeMessage {
                        envelope_id,
                        payload: None,
                    })
                    .with_context(|| "serializing ack message")?;
                    write
                        .send(tungstenite::Message::Text(ack_message.into()))
                        .await
                        .with_context(|| "replying ack message")?;

                    if let Ok(mentioned) = serde_json::from_value::<MentionedPayload>(payload) {
                        let event = mentioned.event;
                        slack_client
                            .send_message(
                                &event.channel,
                                &format!(
                                    "You said: ```{}```",
                                    event.text.unwrap_or_else(String::new)
                                ),
                            )
                            .await
                            .with_context(|| "sending message")?;
                    }
                }
                Err(e) => {
                    println!("Failed to parse websocket frame: {:?}", e);
                }
                Ok(SocketModeMessage::SlashCommands { payload, .. }) => {
                    println!("SlashCommands: {}", payload);
                }
                Ok(SocketModeMessage::Interactive { payload, .. }) => {
                    println!("Interactive: {}", payload);
                }
            },
            _ => println!("unsupported frame"),
        }
    }

    Ok(())
}
