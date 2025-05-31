use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use base64::{Engine as _, engine::general_purpose};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiMessage {
    pub role: String,
    pub parts: Vec<GeminiPart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum GeminiPart {
    Text { 
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>
    },
    InlineData { 
        #[serde(skip_serializing_if = "Option::is_none", rename = "inlineData")]
        inline_data: Option<InlineData>
    },
    FunctionCall {
        #[serde(rename = "functionCall")]
        function_call: FunctionCall
    },
    FunctionResponse {
        #[serde(rename = "functionResponse")]
        function_response: FunctionResponse
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct InlineData {
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub data: String, // base64 encoded
}

impl GeminiPart {
    pub fn text(text: String) -> Self {
        Self::Text {
            text: Some(text),
        }
    }
    
    pub fn pdf(base64_data: String) -> Self {
        Self::InlineData {
            inline_data: Some(InlineData {
                mime_type: "application/pdf".to_string(),
                data: base64_data,
            }),
        }
    }
    
    pub fn function_call(name: String, args: Value) -> Self {
        Self::FunctionCall {
            function_call: FunctionCall {
                name,
                args,
            },
        }
    }
    
    pub fn function_response(name: String, response: Value) -> Self {
        Self::FunctionResponse {
            function_response: FunctionResponse {
                name,
                response,
            },
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiResponse {
    pub candidates: Vec<GeminiCandidate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiCandidate {
    pub content: GeminiContent,
    #[serde(rename = "finishReason")]
    pub finish_reason: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GeminiContent {
    pub parts: Vec<GeminiResponsePart>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum GeminiResponsePart {
    Text { text: String },
    FunctionCall { 
        #[serde(rename = "functionCall")]
        function_call: FunctionCall 
    },
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCall {
    pub name: String,
    pub args: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionResponse {
    pub name: String,
    pub response: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ToolConfig {
    #[serde(rename = "functionCallingConfig")]
    pub function_calling_config: FunctionCallingConfig,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FunctionCallingConfig {
    pub mode: String, // "ANY" or "AUTO"
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SystemInstruction {
    pub parts: Vec<GeminiPart>,
}

#[derive(Clone)]
pub struct GeminiClient {
    client: Client,
    api_key: String,
    base_url: String,
    conversation_history: Vec<GeminiMessage>,
    tools: Vec<FunctionDeclaration>,
    system_instruction: SystemInstruction,
}

/// Information about a tool call made by Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub function_name: String,
    pub arguments: Value,
    pub result: String,
}

/// Detailed response information from Gemini
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiResponseDetails {
    pub content: String,
    pub tool_calls: Vec<ToolCallInfo>,
    pub iterations: i32,
    pub has_tool_calls: bool,
}

impl GeminiClient {
    /// Create a new Gemini client with the provided API key
    pub fn new(api_key: String) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::new();
        let base_url = "https://generativelanguage.googleapis.com/v1beta/models".to_string();
        
        let mut gemini_client = Self {
            client,
            api_key,
            base_url,
            conversation_history: Vec::new(),
            tools: Vec::new(),
            system_instruction: SystemInstruction { parts: Vec::new() },
        };

        // Setup Excel tools
        gemini_client.setup_excel_tools();
        
        // Setup system instruction
        gemini_client.setup_system_instruction();
        
        Ok(gemini_client)
    }

    /// Configure Excel automation tools for Gemini
    fn setup_excel_tools(&mut self) {
        self.tools = vec![
            FunctionDeclaration {
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
            FunctionDeclaration {
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
            FunctionDeclaration {
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
            FunctionDeclaration {
                name: "get_excel_sheet_overview".to_string(),
                description: "Get a complete overview of the current Excel sheet showing all non-empty cells and their values".to_string(),
                parameters: json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
            FunctionDeclaration {
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
        ];
    }

    /// Setup the system instruction for the Excel Copilot (Probably need to be improved)
    fn setup_system_instruction(&mut self) {
        let system_prompt = r#"You are Excel Copilot. You interact with Excel files using tools. Always respond in English.

**TOOLS AVAILABLE:**
- read_excel_cell, write_excel_cell, read_excel_range, get_excel_sheet_overview, set_excel_formula

**CRITICAL RULES:**
1. ALWAYS use tools to interact with Excel - NEVER fake results
2. Start every task with get_excel_sheet_overview
3. NEVER invent cell contents or data
4. If you need info, READ it with tools first
5. Don't talk between toolcalls, just use them
6. Fix formula errors immediately (#NAME?, #REF!, etc.)
7. After writing formulas, re-read the cell to verify

**FORBIDDEN:**
- Simulating tool outputs like "Reading A1... value is 42"
- Pretending you called a function
- Inventing Excel data
- Leaving error values in cells

**PROTOCOL:**
- Need data? → Call appropriate read tool
- Need to write? → Call write_excel_cell  
- Lack context? → READ THE FILE
- See errors? → Fix them immediately

Use tools efficiently. Be direct. No hallucination."#;

        self.system_instruction = SystemInstruction {
            parts: vec![GeminiPart::text(system_prompt.to_string())],
        };
    }

    /// Send a message to Gemini with detailed tool calling support
    pub async fn send_message_with_tools_detailed<F>(&mut self, message: &str, tool_executor: F) -> Result<GeminiResponseDetails, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        // Add user message to conversation history
        let user_message = GeminiMessage {
            role: "user".to_string(),
            parts: vec![GeminiPart::text(message.to_string())],
        };
        self.conversation_history.push(user_message);
        
        // Track all tool calls made during this conversation turn
        let mut all_tool_calls = Vec::new();
        let mut total_iterations = 0;
        
        // Start the conversation turn
        let final_response = self.process_conversation_turn(&tool_executor, &mut all_tool_calls, &mut total_iterations).await?;
        
        // Add the final response to conversation history
        let final_message = GeminiMessage {
            role: "model".to_string(),
            parts: vec![GeminiPart::text(final_response.clone())],
        };
        self.conversation_history.push(final_message);
        
        // Calculate has_tool_calls before moving all_tool_calls
        let has_tool_calls = !all_tool_calls.is_empty();
        
        Ok(GeminiResponseDetails {
            content: final_response,
            tool_calls: all_tool_calls,
            iterations: total_iterations,
            has_tool_calls,
        })
    }

    /// Process a complete conversation turn, handling function calls inline
    async fn process_conversation_turn<F>(&mut self, tool_executor: &F, all_tool_calls: &mut Vec<ToolCallInfo>, total_iterations: &mut i32) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        let max_total_iterations = 50;
        let mut accumulated_text = String::new();
        
        // Working conversation that includes intermediate function calls
        let mut working_conversation = self.conversation_history.clone();
        
        loop {
            *total_iterations += 1;
            if *total_iterations > max_total_iterations {
                return Err(format!("Maximum iterations ({}) reached", max_total_iterations).into());
            }
            
            // Make API call to Gemini
            let url = format!("{}/gemini-2.0-flash:generateContent?key={}", self.base_url, self.api_key);
            
            let request_body = json!({
                "system_instruction": self.system_instruction,
                "contents": working_conversation,
                "tools": [{
                    "functionDeclarations": self.tools
                }],
                "toolConfig": {
                    "functionCallingConfig": {
                        "mode": "AUTO"
                    }
                },
                "generationConfig": {
                    "responseMimeType": "text/plain",
                    "temperature": 0.1,
                    "maxOutputTokens": 16000
                }
            });

            let response = self.client
                .post(&url)
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await?;

            if !response.status().is_success() {
                let error_text = response.text().await?;
                return Err(format!("Gemini API error: {}", error_text).into());
            }

            let gemini_response: GeminiResponse = response.json().await?;
            let candidate = gemini_response.candidates.first()
                .ok_or("No candidate in Gemini response")?;

            // DEBUG: Log what we received from Gemini
            println!("=== GEMINI RESPONSE DEBUG (Iteration {}) ===", *total_iterations);
            println!("Finish reason: {:?}", candidate.finish_reason);
            println!("Number of parts: {}", candidate.content.parts.len());
            for (i, part) in candidate.content.parts.iter().enumerate() {
                match part {
                    GeminiResponsePart::Text { text } => {
                        println!("Part {}: TEXT: {}", i, text.chars().take(100).collect::<String>());
                    }
                    GeminiResponsePart::FunctionCall { function_call } => {
                        println!("Part {}: FUNCTION_CALL: {}", i, function_call.name);
                    }
                }
            }
            println!("==========================================");

            // Process the response parts
            let mut has_function_calls = false;
            let mut text_content = String::new();
            let mut function_calls_in_response = Vec::new();
            
            // Collect all text and function calls from this response
            for part in &candidate.content.parts {
                match part {
                    GeminiResponsePart::Text { text } => {
                        text_content.push_str(text);
                    }
                    GeminiResponsePart::FunctionCall { function_call } => {
                        has_function_calls = true;
                        function_calls_in_response.push(function_call.clone());
                    }
                }
            }
            
            // Add text to accumulated response (this preserves all text from the model)
            if !text_content.trim().is_empty() {
                accumulated_text.push_str(&text_content);
            }
            
            // Add the model's response to working conversation (represents what model said in this turn)
            let model_parts: Vec<GeminiPart> = candidate.content.parts.iter().map(|part| {
                match part {
                    GeminiResponsePart::Text { text } => GeminiPart::text(text.clone()),
                    GeminiResponsePart::FunctionCall { function_call } => {
                        // Include the actual function call, not as text
                        GeminiPart::function_call(function_call.name.clone(), function_call.args.clone())
                    }
                }
            }).collect();
            
            if !model_parts.is_empty() {
                let model_message = GeminiMessage {
                    role: "model".to_string(),
                    parts: model_parts,
                };
                working_conversation.push(model_message);
            }
            
            // If there are function calls, execute them and add results for next iteration
            if has_function_calls {
                println!("=== EXECUTING {} FUNCTION CALLS ===", function_calls_in_response.len());
                
                // Collect all function responses before adding them to the conversation
                let mut function_response_parts = Vec::new();
                
                for function_call in function_calls_in_response {
                    println!("Executing function: {}", function_call.name);
                    
                    // Execute the function
                    let function_result = tool_executor(&function_call.name, &function_call.args).await?;
                    
                    println!("Function result length: {}", function_result.len());
                    
                    // Record the tool call
                    let tool_call_info = ToolCallInfo {
                        function_name: function_call.name.clone(),
                        arguments: function_call.args.clone(),
                        result: function_result.clone(),
                    };
                    all_tool_calls.push(tool_call_info);
                    
                    // Collect the function response part
                    function_response_parts.push(GeminiPart::function_response(
                        function_call.name.clone(),
                        json!({ "content": function_result })
                    ));
                }
                
                // Add all function responses in a single message
                let function_results_message = GeminiMessage {
                    role: "user".to_string(),
                    parts: function_response_parts,
                };
                working_conversation.push(function_results_message);
                
                println!("=== CONTINUING TO NEXT ITERATION ===");
                // Continue to let the model see the function results and potentially continue its response
                continue;
            } else {
                println!("=== NO FUNCTION CALLS - BREAKING ===");
                // No function calls means the model has finished its response
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
        // Clear all conversation history but keep system instruction
        self.conversation_history.clear();
    }

    /// Reset client completely
    pub fn reset_completely(&mut self) {
        // Clear everything and re-setup system instruction
        self.conversation_history.clear();
        self.setup_system_instruction();
    }

    /// Send a message with tool calling support (legacy compatibility)
    pub async fn send_message_with_tools<F>(&mut self, message: &str, tool_executor: F) -> Result<String, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        let details = self.send_message_with_tools_detailed(message, tool_executor).await?;
        Ok(details.content)
    }

    /// Send a simple message without tools
    pub async fn send_message(&mut self, message: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // Add user message to conversation history
        let user_message = GeminiMessage {
            role: "user".to_string(),
            parts: vec![GeminiPart::text(message.to_string())],
        };
        self.conversation_history.push(user_message);

        // Prepare the request without tools but with system instruction
        let url = format!("{}/gemini-2.5-flash-preview-04-17:generateContent?key={}", self.base_url, self.api_key);
        
        let request_body = json!({
            "system_instruction": self.system_instruction,
            "contents": self.conversation_history,
            "generationConfig": {
                "responseMimeType": "text/plain"
            }
        });

        // Send request
        let response = self.client
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await?;

        if response.status().is_success() {
            let gemini_response: GeminiResponse = response.json().await?;
            
            if let Some(candidate) = gemini_response.candidates.first() {
                if let Some(part) = candidate.content.parts.first() {
                    if let GeminiResponsePart::Text { text } = part {
                        let response_text = text.clone();
                        
                        // Add assistant response to conversation history
                        let assistant_message = GeminiMessage {
                            role: "model".to_string(),
                            parts: vec![GeminiPart::text(response_text.clone())],
                        };
                        self.conversation_history.push(assistant_message);
                        
                        return Ok(response_text);
                    }
                }
            }
            
            Err("Empty response from Gemini".into())
        } else {
            let error_text = response.text().await?;
            Err(format!("Gemini API error: {}", error_text).into())
        }
    }

    /// Send a message with PDF attachments
    pub async fn send_message_with_pdf<F>(&mut self, message: &str, pdf_files: Vec<String>, tool_executor: F) -> Result<GeminiResponseDetails, Box<dyn std::error::Error + Send + Sync>>
    where
        F: Fn(&str, &Value) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, Box<dyn std::error::Error + Send + Sync>>> + Send>> + Send + Sync,
    {
        // Create parts for the message
        let mut parts = vec![GeminiPart::text(message.to_string())];
        
        // Add PDF attachments
        for pdf_path in pdf_files {
            match std::fs::read(&pdf_path) {
                Ok(pdf_data) => {
                    let base64_data = general_purpose::STANDARD.encode(pdf_data);
                    parts.push(GeminiPart::pdf(base64_data));
                    println!("Added PDF attachment: {}", pdf_path);
                }
                Err(e) => {
                    println!("Warning: Failed to read PDF {}: {}", pdf_path, e);
                }
            }
        }

        // Add user message with attachments to conversation history
        let user_message = GeminiMessage {
            role: "user".to_string(),
            parts,
        };
        self.conversation_history.push(user_message);
        
        // Track all tool calls made during this conversation turn
        let mut all_tool_calls = Vec::new();
        let mut total_iterations = 0;
        
        // Start the conversation turn
        let final_response = self.process_conversation_turn(&tool_executor, &mut all_tool_calls, &mut total_iterations).await?;
        
        // Add the final response to conversation history
        let final_message = GeminiMessage {
            role: "model".to_string(),
            parts: vec![GeminiPart::text(final_response.clone())],
        };
        self.conversation_history.push(final_message);
        
        // Calculate has_tool_calls before moving all_tool_calls
        let has_tool_calls = !all_tool_calls.is_empty();
        
        Ok(GeminiResponseDetails {
            content: final_response,
            tool_calls: all_tool_calls,
            iterations: total_iterations,
            has_tool_calls,
        })
    }
} 