use anyhow::Result;
use futures_util::StreamExt;
use librespot::discovery::Credentials;
use sha1::{Digest, Sha1};

#[derive(Debug)]
pub struct Transmitter {
    receiver_addr: String,
    http_client: reqwest::Client,
    device_name: String,
}

impl Transmitter {
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
            tracing::debug!("discovery loop");
            tokio::select! {
                credentials = discovery.next() => {
                    tracing::debug!("discovery next");
                    match credentials {
                        Some(credentials) => {
                            self.forward_creds(self.device_name.clone(), credentials).await?;
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

    async fn forward_creds(&mut self, device_name: String, creds: Credentials) -> Result<()> {
        let key = generate_id();
        println!(
            "\n\n****\tyour key is: {:?} - run the following command in discord: !ps {}\t****\n\n",
            key, key
        );
        let resp = self
            .http_client
            .post(self.receiver_addr.clone() + "/api/forward_creds")
            .json(&protocol::ForwardCreds {
                device_name,
                creds,
                key,
            })
            .send()
            .await?;
        let status = resp.status();
        tracing::debug!(?resp, ?status, "forward creds response");
        if status != reqwest::StatusCode::OK {
            anyhow::bail!("forward creds failed with status: {:?}", status);
        }
        Ok(())
    }
}

fn device_id(name: &str) -> String {
    hex::encode(Sha1::digest(name.as_bytes()))
}

fn generate_id() -> String {
    use rand::seq::SliceRandom;

    let base_words = vec![
        "bhrist", "blarf", "brad", "balph", "beer", "bilf", "breek", "buch",
    ];
    let mut rng = rand::thread_rng();

    let word = base_words.choose(&mut rng).unwrap();

    let num = rand::random::<u32>() % 100;

    format!("{}{}", word, num)
}
