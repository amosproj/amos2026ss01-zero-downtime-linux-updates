use anyhow::{Context, Result};
use reqwest::Client;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;
use serde_derive::Deserialize;



// Temporary config struct containing server URL and https_proxy
// This config should not be necessary in the future, when the orchestrator will provide the download manager with the config or its relevant values.
pub struct Config {
    pub server_url: String,
    pub https_proxy: Option<String>,
    pub download_dir: PathBuf,
}


// Temporary struct representing the update information returned by the server
// as mentioned in #15, this struct should be replaced with the complete one once the server is implemented.
#[derive(Debug, Deserialize)]
pub struct UpdateStruct {
    pub os_version: String,
    pub download_url: String,
    pub signature: String,
}


// Builds the async reqwest HTTP client
// If https_proxy is set, it will be used for all requests. Otherwise, reqwest will use https_proxy from the environment variables.
// Returns:
// - Ok(Client) if the client was built successfully
// - Err(anyhow::Error) if there was an error building the client, with a context message indicating the failure reason
pub fn build_http_client(config: &Config) -> Result<Client> {
    
    let mut builder = Client::builder();

    // Reqwest uses https_proxy environment variable by default, we overwrite it if https_proxy is set in config.
    if let Some(proxy_url) = &config.https_proxy {
        println!("Using https proxy: {}", proxy_url);
        let proxy = reqwest::Proxy::https(proxy_url).with_context(|| format!("Failed to set https proxy: {}", proxy_url))?;
        builder = builder.proxy(proxy);
    } else {
        println!("No https proxy set, using environment variables if available");
    }

    builder.build().with_context(|| "Failed building HTTP client")
} 

// Poll the server to check what the available OS version is
// Returns:
// - Ok(Some(UpdateStruct)) if the server responded with a valid update struct
// - Ok(None) if the server responded with a 204 No Content
// - Err(anyhow::Error) if there was an error making the request or parsing the response, with a context message indicating the failure reason
pub async fn check_for_update(client: &Client, config: &Config) -> Result<Option<UpdateStruct>> {

    // Await server response with hard coded timeout. Could be added to config in the future.
    let resp = client.get(&config.server_url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .with_context(|| format!("Failed to reach server at {}", &config.server_url))?;

    // If the server responds with 204 No Content, there is no update available and we return Ok(None).
    if resp.status() == reqwest::StatusCode::NO_CONTENT {
        println!("No update available");
        return Ok(None);
    // If the server responds with any other non-success status code, we return an error.
    } else if !resp.status().is_success() {
        anyhow::bail!("Server at {} responded with status code: {}", &config.server_url, resp.status());
    }

    let update_info = resp.json::<UpdateStruct>()
        .await
        .with_context(|| "Failed to parse server response as UpdateStruct")?;

    Ok(Some(update_info))
}



// Downloads the update from a given UpdateStruct and saves it as "update_<os_version>.bin" to the directory specified in config.download_dir.
// Returns:
// - Ok(PathBuf) with the path to the downloaded file if the download was successful
// - Err(anyhow::Error) if there was an error during the download or file writing process, with a context message indicating the failure reason
pub async fn download_update(client: &Client, update_info: &UpdateStruct, config: &Config) -> Result<PathBuf> {

    tokio::fs::create_dir_all(&config.download_dir)
        .await
        .with_context(|| format!("Failed to create download directory at {:?}", &config.download_dir))?;


    let filename = format!("update_{}.bin", update_info.os_version);
    let file_path = config.download_dir.join(&filename);

    let resp = client.get(&update_info.download_url)
        .timeout(std::time::Duration::from_secs(3600)) // Potentially long download, timeout set to 1 hour for now
        .send()
        .await
        .with_context(|| format!("Failed to download update from {}", &update_info.download_url))?;
    
    if !resp.status().is_success() {
        anyhow::bail!("Failed to download update, server responded with status code: {}", resp.status());
    }

    let mut file = tokio::fs::File::create(&file_path)
        .await
        .with_context(|| format!("Failed to create file at {:?}", &file_path))?;

    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let dl_chunk = chunk.with_context(|| "Failed to read chunk from download stream")?;
        file.write_all(&dl_chunk).await.with_context(|| "Failed to write chunk to file")?;
    }

    Ok(file_path)
}