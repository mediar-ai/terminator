use anyhow::Result;
use crate::cli::{AIProvider, Transport};

pub async fn aichat(transport: Transport, provider: AIProvider,) -> Result<()> {
    match provider {
        AIProvider::Anthropic => super::anthropic::call_anthropic(transport).await,
        AIProvider::OpenAI    => super::openai::call_openai(transport).await,
        AIProvider::Gemini    => super::gemini::call_gemini(transport).await,
    }
}
