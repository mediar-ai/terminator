use anyhow::Result;
use serde_json::{from_str, Value as JsonValue};
use std::collections::HashMap;
use std::io::Write;
use openai_api_rs::v1::{
    api::OpenAIClient,
    chat_completion::{
        self, ChatCompletionRequest, Content, FinishReason, MessageRole, Tool, ToolChoiceType,
        ToolType,
    },
    common::GPT4_O,
    types::{self, FunctionParameters, JSONSchemaDefine, JSONSchemaType},
};
use super::utils::connect_to_mcp;
use crate::workflow_exec::workflow::Transport;
use rmcp::model::CallToolRequestParam;

pub async fn openai_chat(transport: Transport) -> Result<()> {
    println!("🤖 OpenAI GPT-4o Chat Client (Modern API)");
    println!("==========================================");

    dotenvy::dotenv().ok();
    let api_key = match std::env::var("OPENAI_API_KEY") {
        Ok(key) => key,
        Err(_) => {
            println!("❌ OPENAI_API_KEY environment variable not set.");
            return Ok(());
        }
    };
    let mut client = OpenAIClient::builder().with_api_key(api_key).build().unwrap();

    let service = connect_to_mcp(transport).await?;
    if let Some(info) = service.peer_info() {
        println!("✅ Connected to MCP server: {}", info.server_info.name);
    }

    let mcp_tools = service.list_all_tools().await?;
    let openai_tools: Vec<Tool> = mcp_tools
        .into_iter()
        .filter_map(|t| {
            // into the strongly-typed structures openai's library requires.
            let schema = t.input_schema;
            let properties_map = schema.get("properties").and_then(|p| p.as_object())?;
            let required_vec = schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                });

            let mut properties = HashMap::new();
            for (key, value) in properties_map {
                let description = value.get("description").and_then(|d| d.as_str()).map(String::from);
                let property_schema = Box::new(JSONSchemaDefine {
                    schema_type: Some(JSONSchemaType::String),
                    description,
                    ..Default::default()
                });
                properties.insert(key.clone(), property_schema);
            }

            Some(Tool {
                r#type: ToolType::Function,
                function: types::Function {
                    name: t.name.to_string(),
                    description: Some(t.description.unwrap().to_string()),
                    parameters: FunctionParameters {
                        schema_type: JSONSchemaType::Object,
                        properties: Some(properties),
                        required: required_vec,
                    },
                },
            })
        })
        .collect();

    if !openai_tools.is_empty() {
        println!("✅ Found {} tools.", openai_tools.len());
    } else {
        println!("✅ No tools found or parsed.");
    }
    println!("\n💡 Type your command in natural language.");
    println!("Type 'exit' or 'quit' to end the session.\n");

    let mut messages: Vec<chat_completion::ChatCompletionMessage> = Vec::new();

    loop {
        print!("💬 You: ");
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let input = input.trim();

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            break;
        }
        if input.is_empty() {
            continue;
        }

        messages.push(chat_completion::ChatCompletionMessage {
            role: MessageRole::user,
            content: Content::Text(input.to_string()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        });

        println!("🤔 Thinking...");

        loop {
            let mut req_builder = ChatCompletionRequest::new(GPT4_O.to_string(), messages.clone());

            if !openai_tools.is_empty() {
                req_builder = req_builder
                    .tools(openai_tools.clone())
                    .tool_choice(ToolChoiceType::Auto);
            }
            let req = req_builder; 

            let result = client.chat_completion(req).await?;
            let choice = &result.choices[0];
            let response_message = choice.message.clone();

            messages.push(chat_completion::ChatCompletionMessage {
                name: response_message.name,
                role: MessageRole::assistant,
                content: response_message.content.map(Content::Text).unwrap_or(Content::Text("".to_string())),
                tool_calls: response_message.tool_calls.map(|calls| {
                    calls
                        .into_iter()
                        .map(|call| chat_completion::ToolCall {
                            id: call.id,
                            r#type: call.r#type,
                            function: chat_completion::ToolCallFunction {
                                name: call.function.name,
                                arguments: call.function.arguments,
                            },
                        })
                        .collect()
                }),
                tool_call_id: None,
            });

            // check if the model stopped because it wants to call a tool
            if let Some(FinishReason::tool_calls) = choice.finish_reason {
                let tool_calls = choice.message.tool_calls.as_ref().unwrap();
                println!("\n🔧 Executing {} tool(s)...", tool_calls.len());

                for tool_call in tool_calls {
                    let func = &tool_call.function;
                    let args = &func.arguments;
                    println!("  - Calling `{:?}` with args: {:?}", func.name.clone(), args.clone());

                    let arguments_json: JsonValue = from_str(&args.clone().unwrap())?;

                    let tool_result = service
                        .call_tool(CallToolRequestParam {
                            name: func.name.as_ref().unwrap().clone().into(),
                            arguments: arguments_json.as_object().cloned(),
                        })
                        .await;

                    let result_content = match tool_result {
                        Ok(res) => format!("{:?}", res),
                        Err(e) => format!("Error: {e}"),
                    };
                    println!("  ✅ Result: {}", result_content);

                    messages.push(chat_completion::ChatCompletionMessage {
                        role: MessageRole::tool,
                        tool_call_id: Some(tool_call.id.clone()),
                        content: Content::Text(result_content),
                        name: func.name.clone(),
                        tool_calls: None,
                    });
                }
                println!("\n🤔 Processing results...");
                // continue the inner loop to send the tool results back to the model
                continue;
            }

            // If it was a normal text response, print it and break the inner loop
            if let Some(text) = &choice.message.content {
                println!("{}", text);
            }
            break;
        }
    }

    println!("\n👋 Goodbye!");
    service.cancel().await?;
    Ok(())
}

