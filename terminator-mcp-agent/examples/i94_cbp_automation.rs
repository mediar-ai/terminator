use anyhow::Result;
use rmcp::{
    model::CallToolRequestParam,
    object,
    transport::TokioChildProcess,
    ServiceExt,
};
use serde::Serialize;
use tokio::process::Command;

#[derive(Debug, Serialize)]
struct TravelerInfo<'a> {
    first_name: &'a str,
    last_name: &'a str,
    date_of_birth: &'a str, // MM/DD/YYYY
    citizenship: &'a str,
    document_number: &'a str,
}

#[tokio::main]
async fn main() -> Result<()> {
    // ---------------------------------------------------------------------
    // 1. Launch the MCP agent in stdio mode and establish a client session.
    // ---------------------------------------------------------------------
    // Make sure `terminator-mcp-agent` is in your PATH. If it lives elsewhere
    // (e.g. `./target/release/terminator-mcp-agent`), adjust the command path
    // below accordingly.
    let mut cmd = Command::new("terminator-mcp-agent");
    cmd.args(["-t", "stdio"]);

    // Spawn the child-process transport and perform the RMCP handshake.
    let service = ().serve(TokioChildProcess::new(cmd)?).await?;

    println!("✅ Connected to MCP server – ready to automate!");

    // ---------------------------------------------------------------------
    // 2. Define the traveller data we want to query.
    // ---------------------------------------------------------------------
    let traveler = TravelerInfo {
        first_name: "MICKEY",
        last_name: "MOUSE",
        date_of_birth: "01/01/1990",
        citizenship: "CANADA",
        document_number: "A1234567",
    };

    // ---------------------------------------------------------------------
    // 3. (Optional) Launch a browser if one is not already running.
    //    On Windows this could be `chrome`, on Linux perhaps `firefox`.
    //    Comment out if you prefer to attach to an existing browser.
    // ---------------------------------------------------------------------
    let _ = service
        .call_tool(CallToolRequestParam {
            name: "open_application".into(),
            arguments: Some(object!({ "app_name": "chrome" })),
        })
        .await;

    // ---------------------------------------------------------------------
    // 4. Navigate to the I-94 recent-travel-history search form.
    // ---------------------------------------------------------------------
    service
        .call_tool(CallToolRequestParam {
            name: "navigate_browser".into(),
            arguments: Some(object!({
                "url": "https://i94.cbp.dhs.gov/search/recent-search"
            })),
        })
        .await?;

    // The page may take a moment to load – wait for the first input to exist.
    service
        .call_tool(CallToolRequestParam {
            name: "wait_for_element".into(),
            arguments: Some(object!({
                "selector": "name:First (Given) Name",
                "condition": "exists",
                "timeout_ms": 5000
            })),
        })
        .await?;

    // ---------------------------------------------------------------------
    // 5. Fill in each of the required fields.
    // ---------------------------------------------------------------------
    // First (Given) Name
    service
        .call_tool(CallToolRequestParam {
            name: "type_into_element".into(),
            arguments: Some(object!({
                "selector": "name:First (Given) Name",
                "text_to_type": traveler.first_name,
                "clear_before_typing": true
            })),
        })
        .await?;

    // Last (Family) Name / Surname
    service
        .call_tool(CallToolRequestParam {
            name: "type_into_element".into(),
            arguments: Some(object!({
                "selector": "name:Last (Family) Name/Surname",
                "text_to_type": traveler.last_name,
                "clear_before_typing": true
            })),
        })
        .await?;

    // Date of Birth – most browsers expose the full placeholder as the accessible name
    service
        .call_tool(CallToolRequestParam {
            name: "type_into_element".into(),
            arguments: Some(object!({
                "selector": "placeholder:MM/DD/YYYY",
                "text_to_type": traveler.date_of_birth,
                "clear_before_typing": true
            })),
        })
        .await?;

    // Country of Citizenship – a combobox / select element
    service
        .call_tool(CallToolRequestParam {
            name: "select_option".into(),
            arguments: Some(object!({
                "selector": "name:Country of Citizenship",
                "option_name": traveler.citizenship
            })),
        })
        .await?;

    // Document Number – passport or travel document
    service
        .call_tool(CallToolRequestParam {
            name: "type_into_element".into(),
            arguments: Some(object!({
                "selector": "name:Document Number",
                "text_to_type": traveler.document_number,
                "clear_before_typing": true
            })),
        })
        .await?;

    // ---------------------------------------------------------------------
    // 6. Submit the form by clicking the "Continue" button.
    // ---------------------------------------------------------------------
    service
        .call_tool(CallToolRequestParam {
            name: "click_element".into(),
            arguments: Some(object!({
                // Accessible role+name is the most robust selector for buttons
                "selector": "role:Button >> name:Continue"
            })),
        })
        .await?;

    println!("🎉 Form submitted – the I-94 history should now be displayed.");

    // ---------------------------------------------------------------------
    // 7. Always remember to shut down the RMCP session cleanly.
    // ---------------------------------------------------------------------
    service.cancel().await?;

    Ok(())
}