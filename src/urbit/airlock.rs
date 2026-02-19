//! Lightweight Urbit Airlock HTTP client
//!
//! Implements the minimal Airlock protocol needed to poke a Gall agent:
//! 1. Login via POST /~/login with +code → cookie auth
//! 2. Poke via PUT /~/channel/<uid> with action JSON
//! 3. ACK events via SSE stream
//!
//! Reference: <https://docs.urbit.org/manual/id/airlock>

use crate::config::UrbitConfig;
use anyhow::{Context, Result};
use reqwest::Client;
use serde_json::json;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Lightweight Airlock HTTP client for poking Urbit agents
pub struct AirlockClient {
    config: UrbitConfig,
    http: Client,
    channel_id: String,
    next_id: u64,
    connected: bool,
}

impl AirlockClient {
    /// Create a new Airlock client (does not connect yet)
    pub fn new(config: UrbitConfig) -> Self {
        let http = Client::builder()
            .cookie_store(true)
            .build()
            .expect("failed to build reqwest client");

        let channel_id = format!("loraurbit-{}", Uuid::new_v4());

        info!(
            "Airlock client created for ship {} at {}",
            config.ship, config.url
        );

        Self {
            config,
            http,
            channel_id,
            next_id: 1,
            connected: false,
        }
    }

    /// Authenticate with the Urbit ship using the +code
    pub async fn connect(&mut self) -> Result<()> {
        info!("Authenticating with ship {}...", self.config.ship);

        let login_url = format!("{}/~/login", self.config.url);
        let body = format!("password={}", self.config.code);

        let resp = self
            .http
            .post(&login_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .context("failed to send login request")?;

        let status = resp.status();
        if !status.is_success() && !status.is_redirection() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!(
                "login failed with status {}: {}",
                status,
                body_text
            );
        }

        self.connected = true;
        info!(
            "Authenticated with ship {} (channel: {})",
            self.config.ship, self.channel_id
        );
        Ok(())
    }

    /// Connect with retry logic
    pub async fn connect_with_retry(&mut self, max_retries: u32) -> Result<()> {
        let mut attempt = 0u32;
        loop {
            match self.connect().await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempt += 1;
                    if attempt >= max_retries {
                        return Err(e).context(format!(
                            "failed to connect after {} attempts",
                            max_retries
                        ));
                    }
                    let backoff = std::time::Duration::from_secs(2u64.pow(attempt.min(5)));
                    warn!(
                        "Connection attempt {}/{} failed: {}. Retrying in {:?}...",
                        attempt, max_retries, e, backoff
                    );
                    tokio::time::sleep(backoff).await;
                }
            }
        }
    }

    /// Poke a Gall agent with a JSON payload
    pub async fn poke(
        &mut self,
        app: &str,
        mark: &str,
        json_data: serde_json::Value,
    ) -> Result<()> {
        if !self.connected {
            anyhow::bail!("not connected — call connect() first");
        }

        let msg_id = self.next_id;
        self.next_id += 1;

        let channel_url = format!("{}/~/channel/{}", self.config.url, self.channel_id);

        let poke_body = json!([{
            "id": msg_id,
            "action": "poke",
            "ship": self.config.ship,
            "app": app,
            "mark": mark,
            "json": json_data,
        }]);

        debug!(
            "Poking {}/{} (id={}, channel={})",
            app, mark, msg_id, self.channel_id
        );

        let resp = self
            .http
            .put(&channel_url)
            .json(&poke_body)
            .send()
            .await
            .context("failed to send poke")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();

            // If we get a 401/403, try to reconnect
            if status.as_u16() == 401 || status.as_u16() == 403 {
                warn!("Auth expired, attempting reconnect...");
                self.connected = false;
                self.reconnect().await?;
                // Retry the poke once after reconnect
                return self.poke_inner(app, mark, json_data, msg_id).await;
            }

            anyhow::bail!("poke failed with status {}: {}", status, body_text);
        }

        debug!("Poke {} acknowledged", msg_id);

        // Send ACK for any pending events (best effort)
        self.ack_events().await;

        Ok(())
    }

    /// Internal poke (used for retry after reconnect)
    async fn poke_inner(
        &mut self,
        app: &str,
        mark: &str,
        json_data: serde_json::Value,
        msg_id: u64,
    ) -> Result<()> {
        let channel_url = format!("{}/~/channel/{}", self.config.url, self.channel_id);

        let poke_body = json!([{
            "id": msg_id,
            "action": "poke",
            "ship": self.config.ship,
            "app": app,
            "mark": mark,
            "json": json_data,
        }]);

        let resp = self
            .http
            .put(&channel_url)
            .json(&poke_body)
            .send()
            .await
            .context("failed to send poke (retry)")?;

        let status = resp.status();
        if !status.is_success() {
            let body_text = resp.text().await.unwrap_or_default();
            anyhow::bail!("poke retry failed with status {}: {}", status, body_text);
        }

        debug!("Poke {} acknowledged (after reconnect)", msg_id);
        Ok(())
    }

    /// Attempt to reconnect (new channel + re-login)
    async fn reconnect(&mut self) -> Result<()> {
        warn!("Reconnecting to ship {}...", self.config.ship);
        self.channel_id = format!("loraurbit-{}", Uuid::new_v4());
        self.next_id = 1;
        self.connect().await
    }

    /// ACK pending events (best effort, non-blocking)
    ///
    /// After a poke, the ship queues events on the channel's SSE stream.
    /// We need to ACK them to prevent the channel from filling up.
    /// For a poke-only client, we do a quick non-blocking check.
    async fn ack_events(&mut self) {
        let channel_url = format!("{}/~/channel/{}", self.config.url, self.channel_id);

        // Send an ACK for event-id 0 through the current highest
        // Since we're poke-only, we just ACK event 0 proactively
        let ack_id = self.next_id;
        self.next_id += 1;

        let ack_body = json!([{
            "id": ack_id,
            "action": "ack",
            "event-id": 0,
        }]);

        match self
            .http
            .put(&channel_url)
            .json(&ack_body)
            .send()
            .await
        {
            Ok(resp) => {
                if !resp.status().is_success() {
                    debug!("ACK response: {}", resp.status());
                }
            }
            Err(e) => {
                debug!("ACK failed (non-critical): {}", e);
            }
        }
    }

    /// Delete the channel on shutdown (cleanup)
    pub async fn disconnect(&mut self) {
        if !self.connected {
            return;
        }

        let channel_url = format!("{}/~/channel/{}", self.config.url, self.channel_id);

        let delete_id = self.next_id;
        self.next_id += 1;

        let delete_body = json!([{
            "id": delete_id,
            "action": "delete",
        }]);

        match self
            .http
            .put(&channel_url)
            .json(&delete_body)
            .send()
            .await
        {
            Ok(_) => info!("Channel {} cleaned up", self.channel_id),
            Err(e) => debug!("Channel cleanup failed (non-critical): {}", e),
        }

        self.connected = false;
    }

    /// Check if the client is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_airlock_client_creation() {
        let config = UrbitConfig {
            url: "http://localhost:8080".to_string(),
            ship: "zod".to_string(),
            code: "lidlut-tabwed-pillex-ridrup".to_string(),
            agent: "lora-agent".to_string(),
        };

        let client = AirlockClient::new(config);
        assert!(!client.is_connected());
        assert!(client.channel_id.starts_with("loraurbit-"));
        assert_eq!(client.next_id, 1);
    }

    #[test]
    fn test_channel_id_is_unique() {
        let config = UrbitConfig {
            url: "http://localhost:8080".to_string(),
            ship: "zod".to_string(),
            code: "test-code".to_string(),
            agent: "lora-agent".to_string(),
        };

        let client1 = AirlockClient::new(config.clone());
        let client2 = AirlockClient::new(config);
        assert_ne!(client1.channel_id, client2.channel_id);
    }
}
