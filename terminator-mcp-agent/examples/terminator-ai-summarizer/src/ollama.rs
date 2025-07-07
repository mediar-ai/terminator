use anyhow::Result;
use serde_json::Value;
use ollama_rs::{
    Ollama, generation::completion::request::GenerationRequest
};

pub async fn summrize_by_ollama(
        model: &str,
        system_prompt: &str,
        mcp_result: &Value
    ) -> Result<String> {

    let ollama = Ollama::default();
    tracing::info!("sending context to ollama model: {}", model);

    let prompt = format!("{}\n Screen ui element tree: {}", system_prompt, mcp_result);

    let request = GenerationRequest::new(model.to_string(), prompt);
    let response = ollama
        .generate(request)
        .await?;

    tracing::info!("successfully received response from Ollama");

    Ok(response.response)
}

