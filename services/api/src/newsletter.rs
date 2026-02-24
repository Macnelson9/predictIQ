use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::Context;
use serde_json::json;
use tokio::sync::Mutex;

use crate::config::Config;

#[derive(Clone, Default)]
pub struct IpRateLimiter {
    entries: Arc<Mutex<HashMap<String, Vec<Instant>>>>,
}

impl IpRateLimiter {
    pub async fn allow(&self, key: &str, max_requests: usize, window: Duration) -> bool {
        let now = Instant::now();
        let mut map = self.entries.lock().await;
        let entry = map.entry(key.to_string()).or_default();

        entry.retain(|instant| now.duration_since(*instant) <= window);
        if entry.len() >= max_requests {
            return false;
        }

        entry.push(now);
        true
    }
}

pub async fn send_confirmation_email(config: &Config, email: &str, token: &str) -> anyhow::Result<()> {
    let api_key = config
        .sendgrid_api_key
        .as_deref()
        .context("missing SENDGRID_API_KEY")?;
    let from_email = config.from_email.as_deref().context("missing FROM_EMAIL")?;

    let confirm_url = format!(
        "{}/api/v1/newsletter/confirm?token={token}",
        config.base_url.trim_end_matches('/')
    );

    let payload = json!({
        "personalizations": [{ "to": [{ "email": email }] }],
        "from": { "email": from_email },
        "subject": "Confirm your subscription",
        "content": [{
            "type": "text/html",
            "value": format!(
                "<p>Click <a href=\"{confirm_url}\">here</a> to confirm your newsletter subscription.</p>"
            )
        }]
    });

    let response = reqwest::Client::new()
        .post("https://api.sendgrid.com/v3/mail/send")
        .bearer_auth(api_key)
        .json(&payload)
        .send()
        .await
        .context("sendgrid request failed")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("sendgrid returned {status}: {body}");
    }

    Ok(())
}
