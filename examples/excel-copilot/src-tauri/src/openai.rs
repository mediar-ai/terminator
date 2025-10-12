use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;
use crate::locale_utils::{get_decimal_separator, get_locale_info};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAIMessage {
    pub role: String,
    pub content: Vec<OpenAIContent>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum OpenAIContent {
    Text { 
        #[serde(rename = "type")]
        content_type: String,
        text: String,
    },
    ImageUrl { 
        #[serde(rename = "type")]
        content_type: String,
        image_url: ImageUrl,
    },
    File {
        #[serde(rename = "type")]
        content_type: String,
        file: FileReference,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImageUrl {
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileReference {
    pub file_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIResponse {
    pub choices: Vec<OpenAIChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIChoice {
    pub message: OpenAIResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAIResponseMessage {
    pub role: String,
    pub content: Option<String>,
    pub tool_calls: Option<Vec<OpenAIToolCall>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAIToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: OpenAIFunction,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAIFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAITool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: OpenAIFunctionDefinition,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OpenAIFunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

impl OpenAIContent {
    pub fn text(text: String) -> Self {
        Self::Text {
            content_type: "text".to_string(),
            text,
        }
    }
    
    pub fn image_url(url: String) -> Self {
        Self::ImageUrl {
            content_type: "image_url".to_string(),
            image_url: ImageUrl { url },
        }
    }
    
    pub fn file(file_id: String) -> Self {
        Self::File {
            content_type: "file".to_string(),
            file: FileReference { file_id },
        }
    }
}

/// Information about a tool call made by OpenAI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIToolCallInfo {
    pub function_name: String,
    pub arguments: Value,
    pub result: String,
}

/// Detailed response information from OpenAI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAIResponseDetails {
    pub content: String,
    pub tool_calls: Vec<OpenAIToolCallInfo>,
    pub iterations: i32,
    pub has_tool_calls: bool,
}

#[derive(Clone)]
pub struct OpenAIClient {
    client: Client,
    api_key: String,
    base_url: String,
    conversation_history: Vec<OpenAIMessage>,
    tools: Vec<OpenAITool>,
    system_message: String,
    copilot_enabled: bool,
}

impl OpenAIClient {
    /// Create a new OpenAI client with the provided API key
    pub fn new(api_key: String, copilot_enabled: bool) -> Self {
        let client = Client::new();
        let base_url = "https://api.openai.com/v1".to_string();
        
        let mut openai_client = Self {
            client,
            api_key,
            base_url,
            conversation_history: Vec::new(),
            tools: Vec::new(),
            system_message: String::new(),
            copilot_enabled,
        };

        // Setup Excel tools
        openai_client.setup_excel_tools();
        
        // Setup system instruction
        openai_client.setup_system_instruction();
        
        openai_client
    }

    /// Configure Excel automation tools for OpenAI
    fn setup_excel_tools(&mut self) {
        let mut tools = vec![
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "read_excel_cell".to_string(),
                    description: "Read the value from a specific cell in Excel".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "cell_address": {
                                "type": "string",
                                "description": "The cell address in Excel notation (e.g., A1, B2, C10)"
                            }
                        },
                        "required": ["cell_address"]
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "write_excel_cell".to_string(),
                    description: "Write a value to a specific cell in Excel".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "cell_address": {
                                "type": "string",
                                "description": "The cell address in Excel notation (e.g., A1, B2, C10)"
                            },
                            "value": {
                                "type": "string",
                                "description": "The value to write to the cell"
                            }
                        },
                        "required": ["cell_address", "value"]
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "read_excel_range".to_string(),
                    description: "Read values from a range of cells in Excel".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "start_cell": {
                                "type": "string",
                                "description": "The starting cell address (e.g., A1)"
                            },
                            "end_cell": {
                                "type": "string",
                                "description": "The ending cell address (e.g., C5)"
                            }
                        },
                        "required": ["start_cell", "end_cell"]
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "get_excel_sheet_overview".to_string(),
                    description: "Get a complete overview of the current Excel sheet showing all non-empty cells and their values".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "get_excel_ui_context".to_string(),
                    description: "Get the complete UI tree context of the Excel window, including available UI elements, active cell, and element IDs for precise targeting".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "paste_tsv_batch_data".to_string(),
                    description: "Paste batch data in TSV (Tab-Separated Values) format into Excel starting from a specific cell. Use this ONLY for large batch operations like importing data from PDFs or bulk data entry. Always verify safety to avoid overwriting existing data.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "start_cell": {
                                "type": "string",
                                "description": "The starting cell address where to paste the data (e.g., A1, B3)"
                            },
                            "tsv_data": {
                                "type": "string",
                                "description": "The data in TSV format - rows separated by newlines, columns separated by tabs. Example: 'Name\\tAge\\tCity\\nJohn\\t25\\tParis\\nMarie\\t30\\tLyon'"
                            },
                            "verify_safe": {
                                "type": "boolean",
                                "description": "Whether to verify that the target range is empty before pasting (default: true). Set to false only if you're sure you want to overwrite data."
                            }
                        },
                        "required": ["start_cell", "tsv_data"]
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "set_excel_formula".to_string(),
                    description: "Set a formula in an Excel cell".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "cell_address": {
                                "type": "string",
                                "description": "The cell address where to set the formula"
                            },
                            "formula": {
                                "type": "string",
                                "description": "The formula to set (e.g., =SUM(A1:A10), =AVERAGE(B1:B5))"
                            }
                        },
                        "required": ["cell_address", "formula"]
                    }),
                },
            },
        ];

        // Add Copilot tools only if enabled
        if self.copilot_enabled {
            tools.extend(vec![
                OpenAITool {
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionDefinition {
                        name: "send_request_to_excel_copilot".to_string(),
                        description: "Send a request to Microsoft Excel Copilot for advanced tasks like formatting, charts, conditional formatting, data analysis, etc. Note: This will work on currently selected cells or entire sheet.".to_string(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "request": {
                                    "type": "string",
                                    "description": "The request to send to Excel Copilot (e.g., 'Create a bar chart from this data', 'Format these cells with bold and red background', 'Apply conditional formatting to highlight values greater than 100')"
                                }
                            },
                            "required": ["request"]
                        }),
                    },
                },
                OpenAITool {
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionDefinition {
                        name: "format_cells_with_copilot".to_string(),
                        description: "Format Excel cells using Copilot with specific formatting instructions. This will first select the specified range, then use Copilot to format it.".to_string(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "range": {
                                    "type": "string",
                                    "description": "The cell range to format (e.g., A1:C10, B1:D8)"
                                },
                                "format_description": {
                                    "type": "string",
                                    "description": "Description of the formatting to apply (e.g., 'bold text with blue background', 'currency format', 'percentage with 2 decimals')"
                                }
                            },
                            "required": ["range", "format_description"]
                        }),
                    },
                },
                OpenAITool {
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionDefinition {
                        name: "create_chart_with_copilot".to_string(),
                        description: "Create charts in Excel using Copilot. This will first select the specified data range, then ask Copilot to create the chart.".to_string(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "data_range": {
                                    "type": "string",
                                    "description": "The data range for the chart (e.g., A1:B10, B1:D8)"
                                },
                                "chart_type": {
                                    "type": "string",
                                    "description": "Type of chart to create (e.g., 'bar chart', 'line chart', 'pie chart', 'scatter plot')"
                                }
                            },
                            "required": ["data_range", "chart_type"]
                        }),
                    },
                },
                OpenAITool {
                    tool_type: "function".to_string(),
                    function: OpenAIFunctionDefinition {
                        name: "apply_conditional_formatting_with_copilot".to_string(),
                        description: "Apply conditional formatting to Excel cells using Copilot. This will first select the specified range, then apply the conditional formatting.".to_string(),
                        parameters: json!({
                            "type": "object",
                            "properties": {
                                "range": {
                                    "type": "string",
                                    "description": "The cell range to apply conditional formatting (e.g., A1:D20, B1:E15)"
                                },
                                "condition": {
                                    "type": "string",
                                    "description": "The condition for formatting (e.g., 'values greater than 100', 'duplicate values', 'top 10 values')"
                                }
                            },
                            "required": ["range", "condition"]
                        }),
                    },
                },
            ]);
        }

        // Add Google Sheets tools (always available)
        tools.extend(vec![
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "check_google_sheets_availability".to_string(),
                    description: "Check if Google Sheets is open and available with Gemini access. Use this FIRST before any Google Sheets operations to verify availability.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "open_google_sheets_app".to_string(),
                    description: "Open Google Sheets application in browser as a web app. Use this only when user explicitly requests to open a new Google Sheets window.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {},
                        "required": []
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "send_request_to_google_sheets_gemini".to_string(),
                    description: "Send a request to Google Sheets' built-in Gemini for any spreadsheet task. This is the ONLY way to interact with Google Sheets - all operations must go through Gemini. Use for data entry, analysis, formatting, charts, formulas, etc.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "request": {
                                "type": "string",
                                "description": "The request to send to Google Sheets Gemini (e.g., 'Create a table with columns Name, Age, City', 'Analyze this data and create a chart', 'Format the header row with bold text')"
                            }
                        },
                        "required": ["request"]
                    }),
                },
            },
            OpenAITool {
                tool_type: "function".to_string(),
                function: OpenAIFunctionDefinition {
                    name: "send_data_to_google_sheets_gemini".to_string(),
                    description: "Send structured data to Google Sheets Gemini with instructions. Use this to insert tabular data by providing real TSV format (actual tab characters). Gemini will format and insert it appropriately.".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "data": {
                                "type": "string",
                                "description": "The data to send - TSV format with real tab characters (\\t), raw text, or structured data"
                            },
                            "description": {
                                "type": "string",
                                "description": "Instructions for how to handle the data (e.g., 'Create a table with this data and add headers', 'Insert this financial data into the spreadsheet')"
                            }
                        },
                        "required": ["data", "description"]
                    }),
                },
            },
        ]);

        self.tools = tools;
    }

    /// Setup the system instruction for the Excel Copilot
    fn setup_system_instruction(&mut self) {
        // Get locale information
        let _locale_info = get_locale_info();
        let decimal_sep = get_decimal_separator();
        
        // Create locale-specific formatting rules
        let number_format_rules = if decimal_sep == ',' {
            "LOCALE: Comma decimal. Normalize numbers: remove +, thousands separators, convert comma→dot for Excel (e.g., '1.234,56' → '1234.56')"
        } else {
            "LOCALE: Dot decimal. Normalize numbers: remove +, comma thousands separators (e.g., '1,234.56' → '1234.56')"
        };

        let base_tools = r#"TOOLS: read_excel_cell, write_excel_cell, read_excel_range, get_excel_sheet_overview, get_excel_ui_context, paste_tsv_batch_data, set_excel_formula"#;

        let copilot_tools = if self.copilot_enabled {
            ", send_request_to_excel_copilot, format_cells_with_copilot, create_chart_with_copilot, apply_conditional_formatting_with_copilot"
        } else {
            ""
        };

        let copilot_rules = if self.copilot_enabled {
            r#"
You are Excel Copilot, a powerful assistant that can help with a wide range of Excel tasks.

COPILOT CONSTRAINTS:
- OneDrive files only, >= 3 rows, >= 2 cols, headers required, auto-save enabled
- Check file path contains "OneDrive" before Copilot operations
- Be specific in requests: "Format with bold headers, currency column C, alternating rows" not "Format data"

COPILOT USAGE:
- Complex formatting → format_cells_with_copilot
- Charts → create_chart_with_copilot  
- Conditional formatting → apply_conditional_formatting_with_copilot
- Analysis → send_request_to_excel_copilot
- Be precise with copilot, tell him exactly what he should do"#
        } else {
            "\nMicrosoft Excel Copilot: Disabled"
        };

        let google_sheets_rules = r#"

GOOGLE SHEETS COMMUNICATION STRATEGY:
- ALWAYS start with check_google_sheets_availability before any Google Sheets operations
- Use open_google_sheets_app ONLY when user explicitly asks to open NEW Google Sheets
- System now intelligently checks if Gemini panel is already open before clicking "Ask Gemini"

GOOGLE SHEETS PROMPTING BEST PRACTICES:
1. Be DIRECT and ACTION-ORIENTED - tell Gemini exactly what to DO, not what you want to achieve
2. Use SPECIFIC language that Google Sheets understands:
   Good: "Create a table with these columns: Date, Vendor, Amount. Add this data: [data]. Format the Amount column as currency."
   Bad: "Create a table with this data including headers, and format the Amount column as currency"
   
3. BREAK DOWN complex requests into clear steps:
   Good: "Add these 5 rows of data to the sheet: [data]. Then format column C as currency. Then make row 1 bold."
   Bad: "Process this financial data with proper formatting"

4. Use GOOGLE SHEETS VOCABULARY:
   - "Add to the sheet" / "Insert in the spreadsheet"
   - "Format as currency/percentage/date"
   - "Make bold/italic"
   - "Create chart from range A1:C10"
   - "Apply conditional formatting"
   - "Add formula =SUM(A1:A10)"

5. ALWAYS END with action words:
   - "Add it to the sheet"
   - "Apply this formatting"
   - "Create the chart now"
   - "Insert this data"

SUCCESSFUL PROMPTING EXAMPLES:
- Data entry: "Add this data to the spreadsheet starting in cell A1: [TSV data]. Make the first row bold as headers."
- Formatting: "Format column B as currency. Make all text in row 1 bold and centered."
- Charts: "Create a bar chart using data from A1:B10. Add it below the data."
- Calculations: "Add a formula in cell D2 that calculates =B2*C2. Copy it down to row 10."

AVOID THESE PATTERNS (they trigger "not supported"):
- Complex conditional logic requests
- Multi-step operations in one sentence
- Vague terms like "process", "handle", "manage"
- References to external data sources
- Advanced Excel-specific functions

ALL OPERATIONS go through send_request_to_google_sheets_gemini or send_data_to_google_sheets_gemini
For TSV data: provide REAL tab characters, clear instructions
Panel status is checked automatically to avoid unnecessary "Ask Gemini" clicks"#;

        self.system_message = format!(r#"Excel & Google Sheets automation assistant powered by OpenAI GPT-4.1-mini. {}{}{number_format_rules}

RULES:
1. Always use tools, never fake results
2. Start with get_excel_sheet_overview for Excel OR check_google_sheets_availability for Google Sheets
3. TSV paste for batch data, verify_safe=true unless confirmed
4. UI issues → get_excel_ui_context first
5. Fix formula errors immediately

PROTOCOL:
- Read data → read tools (Excel only)
- Write single cells → write_excel_cell (Excel only)
- Need context? → READ THE FILE, before asking user for information
- Bulk data → paste_tsv_batch_data (Excel) OR send_data_to_google_sheets_gemini (Google Sheets)
- Formulas → set_excel_formula (Excel) OR send_request_to_google_sheets_gemini (Google Sheets)
- Google Sheets: Use CLEAR, SIMPLE prompts with action verbs{copilot_rules}{google_sheets_rules}"#, base_tools, copilot_tools, number_format_rules = number_format_rules);
    }

    /// Send a message to OpenAI with detailed tool calling support
    pub async fn send_message_with_tools_detailed<F>(&mut self, message: &str, tool_executor: F) -> Result<OpenAIResponseDetails, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        self.send_message_with_tools_detailed_cancellable(message, tool_executor, None).await
    }

    /// Send a message to OpenAI with detailed tool calling support and cancellation
    pub async fn send_message_with_tools_detailed_cancellable<F>(&mut self, message: &str, tool_executor: F, cancellation_token: Option<CancellationToken>) -> Result<OpenAIResponseDetails, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        // Add user message to conversation history
        let user_message = OpenAIMessage {
            role: "user".to_string(),
            content: vec![OpenAIContent::text(message.to_string())],
        };
        self.conversation_history.push(user_message);
        
        // Track all tool calls made during this conversation turn
        let mut all_tool_calls = Vec::new();
        let mut total_iterations = 0;
        
        // Start the conversation turn
        let final_response = self.process_conversation_turn_cancellable(&tool_executor, &mut all_tool_calls, &mut total_iterations, cancellation_token).await?;
        
        // Add the final response to conversation history
        let final_message = OpenAIMessage {
            role: "assistant".to_string(),
            content: vec![OpenAIContent::text(final_response.clone())],
        };
        self.conversation_history.push(final_message);
        
        // Calculate has_tool_calls before moving all_tool_calls
        let has_tool_calls = !all_tool_calls.is_empty();
        
        Ok(OpenAIResponseDetails {
            content: final_response,
            tool_calls: all_tool_calls,
            iterations: total_iterations,
            has_tool_calls,
        })
    }

    /// Process a complete conversation turn, handling function calls inline
    async fn process_conversation_turn<F>(&mut self, tool_executor: &F, all_tool_calls: &mut Vec<OpenAIToolCallInfo>, total_iterations: &mut i32) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        self.process_conversation_turn_cancellable(tool_executor, all_tool_calls, total_iterations, None).await
    }

    /// Process a complete conversation turn with cancellation support
    async fn process_conversation_turn_cancellable<F>(&mut self, tool_executor: &F, all_tool_calls: &mut Vec<OpenAIToolCallInfo>, total_iterations: &mut i32, cancellation_token: Option<CancellationToken>) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        let max_total_iterations = 50;
        let mut accumulated_text = String::new();
        
        // Working conversation that includes intermediate function calls
        let mut working_conversation = self.conversation_history.clone();
        
        loop {
            // Check for cancellation before starting iteration
            if let Some(ref token) = cancellation_token {
                if token.is_cancelled() {
                    return Err("Request was cancelled by user".into());
                }
            }

            *total_iterations += 1;
            if *total_iterations > max_total_iterations {
                return Err(format!("Maximum iterations ({}) reached", max_total_iterations).into());
            }
            
            // Prepare messages for OpenAI API
            let mut api_messages = vec![
                json!({
                    "role": "system",
                    "content": self.system_message
                })
            ];
            
            for msg in &working_conversation {
                if msg.content.len() == 1 && matches!(msg.content[0], OpenAIContent::Text { .. }) {
                    // Simple text message
                    if let OpenAIContent::Text { text, .. } = &msg.content[0] {
                        api_messages.push(json!({
                            "role": msg.role,
                            "content": text
                        }));
                    }
                } else {
                    // Complex message with multiple parts (text + files/images)
                    let content_parts: Vec<Value> = msg.content
                        .iter()
                        .map(|c| match c {
                            OpenAIContent::Text { text, .. } => json!({
                                "type": "text",
                                "text": text
                            }),
                            OpenAIContent::ImageUrl { image_url, .. } => json!({
                                "type": "image_url",
                                "image_url": {
                                    "url": image_url.url
                                }
                            }),
                            OpenAIContent::File { file, .. } => json!({
                                "type": "file",
                                "file": {
                                    "file_id": file.file_id
                                }
                            }),
                        })
                        .collect();
                    
                    api_messages.push(json!({
                        "role": msg.role,
                        "content": content_parts
                    }));
                }
            }
            
            // Make API call to OpenAI
            let url = format!("{}/chat/completions", self.base_url);
            
            let request_body = json!({
                "model": "gpt-4.1-mini",
                "messages": api_messages,
                "tools": self.tools,
                "tool_choice": "auto",
                "temperature": 0.1,
                "max_tokens": 4000
            });

            let response = self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", self.api_key))
                .json(&request_body)
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                return Err(format!("OpenAI API error: {}", error_text).into());
            }

            let openai_response: OpenAIResponse = response.json().await?;
            let choice = openai_response.choices.first()
                .ok_or("No choice in OpenAI response")?;

            println!("=== OPENAI RESPONSE DEBUG (Iteration {}) ===", *total_iterations);
            println!("Finish reason: {:?}", choice.finish_reason);
            println!("Content: {:?}", choice.message.content);
            println!("Tool calls: {:?}", choice.message.tool_calls.as_ref().map(|tc| tc.len()));
            println!("==========================================");

            // Process the response
            let mut has_tool_calls = false;
            let mut text_content = String::new();
            let mut tool_calls_in_response = Vec::new();
            
            // Get text content
            if let Some(content) = &choice.message.content {
                text_content = content.clone();
            }
            
            // Get tool calls
            if let Some(tool_calls) = &choice.message.tool_calls {
                has_tool_calls = true;
                tool_calls_in_response = tool_calls.clone();
            }
            
            // Add text to accumulated response
            if !text_content.trim().is_empty() {
                accumulated_text.push_str(&text_content);
            }
            
            // Add the assistant's response to working conversation
            let assistant_content = if !text_content.is_empty() {
                vec![OpenAIContent::text(text_content)]
            } else {
                vec![OpenAIContent::text("Using tools...".to_string())]
            };
            
            let assistant_message = OpenAIMessage {
                role: "assistant".to_string(),
                content: assistant_content,
            };
            working_conversation.push(assistant_message);
            
            // If there are tool calls, execute them and add results for next iteration
            if has_tool_calls {
                println!("=== EXECUTING {} TOOL CALLS ===", tool_calls_in_response.len());
                
                let mut tool_responses = Vec::new();
                
                for tool_call in tool_calls_in_response {
                    // Check for cancellation before executing function
                    if let Some(ref token) = cancellation_token {
                        if token.is_cancelled() {
                            return Err("Request was cancelled by user".into());
                        }
                    }

                    println!("Executing function: {}", tool_call.function.name);
                    
                    // Parse arguments
                    let args: Value = serde_json::from_str(&tool_call.function.arguments)
                        .map_err(|e| format!("Failed to parse tool arguments: {}", e))?;
                    
                    // Execute the function
                    let function_result = tool_executor(&tool_call.function.name, &args).await?;
                    
                    println!("Function result length: {}", function_result.len());
                    
                    // Record the tool call
                    let tool_call_info = OpenAIToolCallInfo {
                        function_name: tool_call.function.name.clone(),
                        arguments: args,
                        result: function_result.clone(),
                    };
                    all_tool_calls.push(tool_call_info);
                    
                    // Prepare tool response for API
                    tool_responses.push(format!("Function '{}' result: {}", tool_call.function.name, function_result));
                }
                
                // Add tool results as user message
                let tool_results_message = OpenAIMessage {
                    role: "user".to_string(),
                    content: vec![OpenAIContent::text(tool_responses.join("\n\n"))],
                };
                working_conversation.push(tool_results_message);
                
                println!("=== CONTINUING TO NEXT ITERATION ===");
                continue;
            } else {
                println!("=== NO TOOL CALLS - BREAKING ===");
                break;
            }
        }
        
        // Return the accumulated text from all iterations
        let final_response = if accumulated_text.trim().is_empty() {
            if !all_tool_calls.is_empty() {
                format!("Task completed successfully. {} tools executed.", all_tool_calls.len())
            } else {
                "Task completed.".to_string()
            }
        } else {
            accumulated_text.trim().to_string()
        };
        
        Ok(final_response)
    }

    /// Clear conversation history
    pub fn clear_conversation(&mut self) {
        self.conversation_history.clear();
    }

    /// Reset client completely
    pub fn reset_completely(&mut self) {
        self.conversation_history.clear();
        self.setup_system_instruction();
    }

    /// Upload a file to OpenAI for user data purposes
    async fn upload_file_for_user_data(&self, file_path: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let file_data = std::fs::read(file_path)?;
        let file_name = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("document.pdf");

        // Create multipart form data
        let form = reqwest::multipart::Form::new()
            .part("purpose", reqwest::multipart::Part::text("user_data"))
            .part("file", reqwest::multipart::Part::bytes(file_data)
                .file_name(file_name.to_string())
                .mime_str("application/pdf")?);

        let response = self.client
            .post("https://api.openai.com/v1/files")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("File upload failed: {}", error_text).into());
        }

        let upload_response: Value = response.json().await?;
        let file_id = upload_response["id"].as_str()
            .ok_or("No file ID in upload response")?
            .to_string();

        Ok(file_id)
    }

    /// Send a message with PDF attachments using File Upload API
    pub async fn send_message_with_pdf<F>(&mut self, message: &str, pdf_files: Vec<String>, tool_executor: F) -> Result<OpenAIResponseDetails, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        // Create content for the message
        let mut content = vec![OpenAIContent::text(message.to_string())];
        
        // Upload PDF files and add them as file references
        for pdf_path in &pdf_files {
            match self.upload_file_for_user_data(pdf_path).await {
                Ok(file_id) => {
                    content.push(OpenAIContent::file(file_id.clone()));
                    println!("Uploaded and added PDF attachment: {} (file_id: {})", pdf_path, file_id);
                }
                Err(e) => {
                    println!("Warning: Failed to upload PDF {}: {}", pdf_path, e);
                    // Continue without this file - OpenAI will inform user about inability to read PDFs
                }
            }
        }

        // Add user message with attachments to conversation history
        let user_message = OpenAIMessage {
            role: "user".to_string(),
            content,
        };
        self.conversation_history.push(user_message);
        
        // Track all tool calls made during this conversation turn
        let mut all_tool_calls = Vec::new();
        let mut total_iterations = 0;
        
        // Start the conversation turn
        let final_response = self.process_conversation_turn(&tool_executor, &mut all_tool_calls, &mut total_iterations).await?;
        
        // Add the final response to conversation history
        let final_message = OpenAIMessage {
            role: "assistant".to_string(),
            content: vec![OpenAIContent::text(final_response.clone())],
        };
        self.conversation_history.push(final_message);
        
        // Calculate has_tool_calls before moving all_tool_calls
        let has_tool_calls = !all_tool_calls.is_empty();
        
        Ok(OpenAIResponseDetails {
            content: final_response,
            tool_calls: all_tool_calls,
            iterations: total_iterations,
            has_tool_calls,
        })
    }
}