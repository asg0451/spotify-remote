use anyhow::Result;
use futures_util::StreamExt;
use librespot::discovery::Credentials;
use reqwest::StatusCode;
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub struct Forwarder {
    receiver_addr: String,
    http_client: reqwest::Client,
    device_name: String,
}

impl Forwarder {
    pub async fn new(receiver_addr: String, device_name: String) -> Result<Self> {
        let http_client = reqwest::ClientBuilder::new()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(5))
            .build()?;
        Ok(Self {
            http_client,
            device_name,
            receiver_addr,
        })
    }

    pub async fn run(mut self) -> Result<()> {
        // pretend to be a spotify receiver to grab credentials

        let device_id = device_id(&self.device_name);

        let mut discovery = librespot::discovery::Discovery::builder(device_id)
            .name(self.device_name.clone())
            .launch()?;

        tracing::debug!("Starting discovery loop");

        loop {
            tokio::select! {
                credentials = discovery.next() => {
                    match credentials {
                        Some(credentials) => {
                            self.forward_creds(credentials).await?;
                            tracing::debug!("forwarded");
                        },
                        None => {
                            anyhow::bail!("Discovery stopped unexpectedly");
                        }
                    }
                },
                 _ = tokio::signal::ctrl_c() => {
                    break;
                },
                else => break,
            }
        }
        tracing::info!("Gracefully shutting down");

        Ok(())
    }

    async fn forward_creds(&mut self, creds: Credentials) -> Result<()> {
        // retry if the code is 409, as that means we picked a key that was already in use
        let (key, status) = loop {
            let key = generate_id();
            let status = self
                .perform_forward_creds_req(creds.clone(), key.clone())
                .await?;
            match status {
                StatusCode::CONFLICT => {
                    tracing::debug!("key conflict, retrying");
                }
                _ => break (key, status),
            }
        };

        println!(
            "\n\n****\tyour key is: {:?} - run the following command in discord: /play_spotify {}\t****\n\n",
            key, key
        );

        if status != reqwest::StatusCode::OK {
            anyhow::bail!("forward creds failed with status: {:?}", status);
        }
        Ok(())
    }

    async fn perform_forward_creds_req(
        &mut self,
        creds: Credentials,
        key: String,
    ) -> Result<StatusCode> {
        let resp = self
            .http_client
            .post(self.receiver_addr.clone() + "/api/forward_creds")
            .json(&protocol::ForwardCreds {
                device_name: self.device_name.clone(),
                creds,
                key,
            })
            .send()
            .await?;
        let status = resp.status();
        tracing::debug!(?resp, ?status, "forward creds response");
        Ok(status)
    }
}

fn device_id(name: &str) -> String {
    hex::encode(Sha1::digest(name.as_bytes()))
}

fn generate_id() -> String {
    use once_cell::sync::Lazy;
    use rand::seq::SliceRandom;

    static BASE_WORDS: Lazy<Vec<&'static str>> =
        Lazy::new(|| vec!["brad", "bro", "beer", "buck", "beans", "bird", "brain"]);

    let mut rng = rand::thread_rng();

    let word = BASE_WORDS.choose(&mut rng).unwrap();

    let num = rand::random::<u32>() % 100;

    format!("{}{}", word, num)
}
