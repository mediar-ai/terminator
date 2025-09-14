use anyhow::Result;
use serde_json::Value;
use serde_json::json;
use std::io::Write;

use gemini_rs::{
    Client,
    types::{Content, FunctionDeclaration,
        FunctionCall, Part, Role, Tools
    },
};
use rmcp::model::CallToolRequestParam;

use super::utils::connect_to_mcp;
use crate::workflow_exec::workflow::Transport;

pub async fn gemini_chat(transport: Transport) -> Result<()> {
    println!("🤖 Gemini AI Chat Client");
    println!("==========================================");

    dotenvy::dotenv().ok();
    if std::env::var("GEMINI_API_KEY").is_err() {
        println!("❌ GEMINI_API_KEY or environment variable not set.");
        println!("Please set one in a .env file or export it.");
        println!("  export GEMINI_API_KEY='your-api-key-here'");
        return Ok(());
    }

    let service = connect_to_mcp(transport).await?;

    if let Some(info) = service.peer_info() {
        println!("✅ Connected to MCP server: {}", info.server_info.name);
    }

    let mcp_tools = service.list_all_tools().await?;
    let gemini_fn_declarations: Vec<FunctionDeclaration> = mcp_tools
        .into_iter()
        .map(|t| FunctionDeclaration {
            name: t.name.to_string(),
            description: t.description.unwrap().to_string(),
            parameters: Value::Object(t.input_schema.as_ref().clone()),
        })
        .collect();

    let gemini_tools = if gemini_fn_declarations.is_empty() {
        None
    } else {
        Some(vec![Tools {
            function_declarations: Some(gemini_fn_declarations),
            google_search: None,
            code_execution: None,
        }])
    };

    if let Some(tools) = &gemini_tools {
        if let Some(decls) = &tools[0].function_declarations {
            println!("✅ Found {} tools.", decls.len());
        }
    } else {
        println!("✅ No tools found or parsed.");
    }

    println!("\n💡 Type your command in natural language. Examples:");
    println!("  - 'Open notepad and type hello world'");
    println!("  - 'Take a screenshot of the desktop'");
    println!("\nType 'exit' or 'quit' to end the session.");
    println!("========================================================================================\n");

    let mut messages: Vec<Content> = Vec::new();
    // singleton client
    let client = Client::instance();
    // model that supports tool calling
    let model_name = "gemini-2.5-flash-preview-04-17";

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

        messages.push(Content {
            role: Role::User,
            parts: vec![Part {
                text: Some(input.to_string()),
                ..Default::default()
            }],
        });

        println!("🤔 Thinking...");

        loop {
            let mut req = client.generate_content(model_name);

            if let Some(tools) = &gemini_tools {
                req.tools(tools.clone());
            }

            req.body.contents = messages.clone();

            let response = match req.await {
                Ok(resp) => resp,
                Err(e) => {
                    println!("❌ Error calling Gemini API: {e}");
                    messages.pop();
                    break;
                }
            };

            let candidate = match response.candidates.get(0) {
                Some(c) => c,
                None => {
                    println!("🤖 No response from model.");
                    break;
                }
            };

            messages.push(candidate.content.clone());

            let mut function_calls_to_execute = Vec::new();
            let mut has_text_response = false;

            for part in &candidate.content.parts {
                if let Some(text) = &part.text {
                    print!("{}", text);
                    has_text_response = true;
                }
                if let Some(fc) = &part.function_call {
                    function_calls_to_execute.push(fc.clone());
                }
            }
            if has_text_response {
                println!();
            }

            if function_calls_to_execute.is_empty() {
                break;
            }

            println!(
                "\n🔧 Executing {} tool(s)...",
                function_calls_to_execute.len()
            );
            let mut tool_results: Vec<Part> = Vec::new();

            for fc in function_calls_to_execute {
                println!("  - Calling `{}` with args: {}", fc.name, fc.args);

                let result = service
                    .call_tool(CallToolRequestParam {
                        name: fc.name.clone().into(),
                        arguments: fc.args.as_object().cloned(),
                    })
                    .await;

                let result_content = match result {
                    Ok(res) => json!({ "result": res }),
                    Err(e) => json!({ "error": format!("{e}") }),
                };

                println!("  ✅ Result: {}", result_content.to_string());

                tool_results.push(Part {
                    function_call: Some(FunctionCall {
                        id: None,
                        name: fc.name,
                        args: result_content,
                    }),
                    ..Default::default()
                });
            }

            messages.push(Content {
                role: Role::User,
                parts: tool_results,
            });

            println!("\n🤔 Processing results...");
        }
    }

    println!("👋 Goodbye!");
    service.cancel().await?;
    Ok(())
}
