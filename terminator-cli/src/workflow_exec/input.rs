use crate::cli::InputType;

pub async fn fetch_remote_content(url: &str) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let res = client
        .get(url)
        .header("User-Agent", "terminator-cli-workflow/1.0")
        .send()
        .await?;
    if !res.status().is_success() {
        return Err(anyhow::anyhow!(
            "HTTP request failed: {} for {}",
            res.status(),
            url
        ));
    }
    Ok(res.text().await?)
}

pub async fn read_local_file(path: &str) -> anyhow::Result<String> {
    use std::path::Path;
    use tokio::fs;

    let p = Path::new(path);
    if !p.exists() {
        return Err(anyhow::anyhow!("File not found: {}", p.display()));
    }
    if !p.is_file() {
        return Err(anyhow::anyhow!("Not a file: {}", p.display()));
    }

    fs::read_to_string(p).await.map_err(|e| e.into())
}

pub fn convert_gist_to_raw_url(gist_url: &str) -> anyhow::Result<String> {
    if !gist_url.starts_with("https://gist.github.com/") {
        return Err(anyhow::anyhow!("Invalid GitHub gist URL format"));
    }

    let raw_url = gist_url.replace(
        "https://gist.github.com/",
        "https://gist.githubusercontent.com/",
    );

    Ok(if raw_url.ends_with("/raw") {
        raw_url
    } else {
        format!("{raw_url}/raw")
    })
}

pub fn determine_input_type(input: &str, specified_type: InputType) -> InputType {
    match specified_type {
        InputType::Auto => {
            if input.starts_with("https://gist.github.com/") {
                InputType::Gist
            } else if input.starts_with("https://gist.githubusercontent.com/")
                || input.starts_with("http://")
                || input.starts_with("https://")
            {
                InputType::Raw
            } else {
                InputType::File
            }
        }
        other => other,
    }
}
