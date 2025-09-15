use anyhow::Result;
use crate::{
    workflow_exec::workflow::Transport,
    cli::AIProvider,
};

pub async fn aichat(transport: Transport, provider: AIProvider,) -> Result<()> {
    match provider {
        AIProvider::Anthropic => super::anthropic::anthropic_chat(transport).await,
        AIProvider::OpenAI    => super::openai::openai_chat(transport).await,
        AIProvider::Gemini    => super::gemini::gemini_chat(transport).await,
    }
}
