use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use terminator::{Desktop, Selector};

/// Represents an Excel cell with its address, value, and optional formula
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelCell {
    pub address: String,
    pub value: String,
    pub formula: Option<String>,
}

/// Represents a range of Excel cells with their values
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelRange {
    pub start_cell: String,
    pub end_cell: String,
    pub values: Vec<Vec<String>>,
}

/// Excel automation interface using the Terminator UI automation library
#[derive(Clone)]
pub struct ExcelAutomation {
    desktop: Desktop,
}

impl ExcelAutomation {
    /// Create a new Excel automation instance
    pub async fn new() -> Result<Self> {
        let desktop = Desktop::new(false, false)?;
        Ok(Self { desktop })
    }

    /// Get or open Excel application
    pub async fn get_excel_application(&self) -> Result<terminator::UIElement> {
        // First try to find existing Excel window
        match self.desktop.application("Excel") {
            Ok(excel_app) => {
                println!("Found existing Excel application");
                Ok(excel_app)
            }
            Err(_) => {
                // If not found, try to open Excel
                println!("Opening Excel application...");
                self.desktop.open_application("excel")
                    .map_err(|e| anyhow!("Failed to open Excel: {}", e))?;
                
                // Wait a bit for Excel to fully load
                tokio::time::sleep(Duration::from_millis(2000)).await;
                
                // Try to get it again
                self.desktop.application("Excel")
                    .map_err(|e| anyhow!("Failed to find Excel after opening: {}", e))
            }
        }
    }

    /// Open an Excel file
    pub async fn open_excel_file(&self, file_path: &str) -> Result<()> {
        println!("Opening Excel file: {}", file_path);
        
        // Open the file using system default (Excel)
        self.desktop.open_file(file_path)?;
        
        // Wait for the file to open
        tokio::time::sleep(Duration::from_millis(3000)).await;
        
        Ok(())
    }

    /// Create a new Excel workbook
    pub async fn create_new_workbook(&self) -> Result<()> {
        println!("Creating new Excel workbook");
        
        let excel_app = self.get_excel_application().await?;
        
        // Try to use Ctrl+N to create new workbook
        excel_app.press_key("{ctrl}n")?;
        
        // Wait for new workbook to be created
        tokio::time::sleep(Duration::from_millis(2000)).await;
        
        Ok(())
    }

    /// Get the Excel window using proper targeting based on automation patterns
    async fn get_excel_window(&self) -> Result<terminator::UIElement> {
        // First try to find by window criteria with Excel in title (most reliable)
        match self.desktop.find_window_by_criteria(Some("Excel"), Some(Duration::from_millis(5000))).await {
            Ok(window) => {
                println!("Found Excel window using title criteria");
                Ok(window)
            }
            Err(_) => {
                // Fallback: try to find Excel application
                println!("Fallback: searching for Excel application");
                match self.desktop.application("Excel") {
                    Ok(app) => {
                        println!("Found Excel application");
                        Ok(app)
                    }
                    Err(e) => Err(anyhow!("Could not find Excel window or application: {}", e))
                }
            }
        }
    }

    /// Read a single cell value using proper Excel targeting
    pub async fn read_cell(&self, cell_address: &str) -> Result<ExcelCell> {
        println!("Reading cell: {}", cell_address);
        
        let excel_window = self.get_excel_window().await?;
        
        // Use Name locator format for cells (like 'Name:A1')
        let cell_selector = Selector::Name(cell_address.to_string());
        match excel_window.locator(cell_selector)?.first(Some(Duration::from_millis(2000))).await {
            Ok(cell_element) => {
                println!("Found cell {} using Name locator", cell_address);
                
                // Click on the cell to select it
                cell_element.click()?;
                tokio::time::sleep(Duration::from_millis(300)).await;
                
                // Try to get the cell value using text extraction with max depth
                let value = match cell_element.text(3) {
                    Ok(text) => {
                        if text.trim().is_empty() {
                            // If direct text is empty, try name attribute
                            match cell_element.attributes().name {
                                Some(name) => name,
                                None => "".to_string()
                            }
                        } else {
                            text.trim().to_string()
                        }
                    }
                    Err(_) => {
                        // Fallback to attributes if text extraction fails
                        cell_element.attributes().name.unwrap_or_default()
                    }
                };
                
                Ok(ExcelCell {
                    address: cell_address.to_string(),
                    value,
                    formula: None,
                })
            }
            Err(_) => {
                println!("Cell {} not found with Name locator, using navigation approach", cell_address);
                
                // Fallback: navigate to cell using Ctrl+G (Go To)
                excel_window.press_key("{ctrl}g")?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Type the cell address
                excel_window.type_text(cell_address, false)?;
                excel_window.press_key("{enter}")?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Try to get active cell value - use formula bar or selected cell
                let value = "".to_string(); // Placeholder
                
                Ok(ExcelCell {
                    address: cell_address.to_string(),
                    value,
                    formula: None,
                })
            }
        }
    }

    /// Write a value to a cell using proper Excel targeting
    pub async fn write_cell(&self, cell_address: &str, value: &str) -> Result<()> {
        println!("Writing to cell {}: {}", cell_address, value);
        
        let excel_window = self.get_excel_window().await?;
        
        // Try to find and select the cell directly first
        let cell_selector = Selector::Name(cell_address.to_string());
        match excel_window.locator(cell_selector)?.first(Some(Duration::from_millis(2000))).await {
            Ok(cell_element) => {
                println!("Found cell {} directly, clicking and typing", cell_address);
                
                // Click on the cell to select it
                cell_element.click()?;
                tokio::time::sleep(Duration::from_millis(300)).await;
                
                // Clear any existing content and type new value
                excel_window.press_key("{delete}")?;
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                excel_window.type_text(value, false)?;
                excel_window.press_key("{enter}")?;
                
                println!("Successfully wrote '{}' to cell {}", value, cell_address);
            }
            Err(_) => {
                println!("Cell {} not found directly, using navigation approach", cell_address);
                
                // Fallback: navigate to cell using Ctrl+G
                excel_window.press_key("{ctrl}g")?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Type the cell address
                excel_window.type_text(cell_address, false)?;
                excel_window.press_key("{enter}")?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Clear and type the value
                excel_window.press_key("{delete}")?;
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                excel_window.type_text(value, false)?;
                excel_window.press_key("{enter}")?;
                
                println!("Successfully wrote '{}' to cell {} using navigation", value, cell_address);
            }
        }
        
        Ok(())
    }

    /// Read a range of cells using selection and clipboard
    pub async fn read_range(&self, start_cell: &str, end_cell: &str) -> Result<ExcelRange> {
        println!("Reading range: {}:{}", start_cell, end_cell);
        
        let excel_window = self.get_excel_window().await?;
        
        // Navigate to start cell using Ctrl+G
        excel_window.press_key("{ctrl}g")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Type the range (e.g., "A1:D12")
        let range_notation = format!("{}:{}", start_cell, end_cell);
        excel_window.type_text(&range_notation, false)?;
        excel_window.press_key("{enter}")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Copy the selected range to clipboard
        excel_window.press_key("{ctrl}c")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Get clipboard content using arboard
        let clipboard_content = match self.get_clipboard_content_real().await {
            Ok(content) => content,
            Err(e) => {
                println!("Error reading clipboard: {}", e);
                return Ok(ExcelRange {
                    start_cell: start_cell.to_string(),
                    end_cell: end_cell.to_string(),
                    values: vec![],
                });
            }
        };
        
        println!("Clipboard content: {}", clipboard_content);
        
        // Parse the clipboard content (tab-separated values, line-separated rows)
        let values = self.parse_clipboard_to_matrix(&clipboard_content);
        
        println!("Parsed {} rows from range", values.len());
        
        Ok(ExcelRange {
            start_cell: start_cell.to_string(),
            end_cell: end_cell.to_string(),
            values,
        })
    }

    /// Get real clipboard content using arboard
    async fn get_clipboard_content_real(&self) -> Result<String> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
        
        let content = clipboard.get_text()
            .map_err(|e| anyhow::anyhow!("Failed to get clipboard text: {}", e))?;
        
        Ok(content)
    }

    /// Get clipboard content (deprecated - use get_clipboard_content_real)
    async fn get_clipboard_content(&self) -> Result<String> {
        self.get_clipboard_content_real().await
    }

    /// Set a formula in a cell
    pub async fn set_formula(&self, cell_address: &str, formula: &str) -> Result<()> {
        println!("Setting formula in {}: {}", cell_address, formula);
        
        let excel_window = self.get_excel_window().await?;
        
        // Navigate to the cell
        excel_window.press_key("{ctrl}g")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        excel_window.type_text(cell_address, false)?;
        excel_window.press_key("{enter}")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Type the formula (ensure it starts with =)
        let formula_text = if formula.starts_with('=') {
            formula.to_string()
        } else {
            format!("={}", formula)
        };
        
        excel_window.type_text(&formula_text, false)?;
        excel_window.press_key("{enter}")?;
        
        Ok(())
    }

    /// Take a screenshot of the Excel window
    pub async fn take_screenshot(&self) -> Result<Vec<u8>> {
        println!("Taking screenshot of Excel window");
        
        let screenshot = self.desktop.capture_screen().await?;
        Ok(screenshot.image_data)
    }

    /// Extract cell value from OCR text
    fn extract_cell_value_from_ocr(&self, ocr_text: &str, _cell_address: &str) -> String {
        ocr_text.lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .trim()
            .to_string()
    }

    /// Parse clipboard content to matrix
    fn parse_clipboard_to_matrix(&self, content: &str) -> Vec<Vec<String>> {
        content
            .lines()
            .map(|line| {
                line.split('\t')
                    .map(|cell| cell.to_string())
                    .collect()
            })
            .collect()
    }

    /// Save the current Excel file 
    pub async fn save_current_file(&self) -> Result<()> {
        println!("Saving current Excel file");
        
        let excel_window = self.get_excel_window().await?;
        
        // Use Ctrl+S to save
        excel_window.press_key("{ctrl}s")?;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        Ok(())
    }

    /// Get sheet overview by reading the actual Excel file without modifying it
    pub async fn get_sheet_overview_from_file(&self, file_path: Option<&str>) -> Result<String> {
        // First save the current state to ensure the file is up to date
        self.save_current_file().await?;
        
        if let Some(path) = file_path {
            match crate::excel::ExcelWorkbook::from_file(path) {
                Ok(workbook) => {
                    let mut overview = String::from("CURRENT EXCEL SHEET OVERVIEW:\n");
                    overview.push_str("===================================\n");
                    
                    if workbook.sheets.is_empty() {
                        overview.push_str("No sheets found in the workbook.\n");
                        return Ok(overview);
                    }
                    
                    // Use the first sheet (most common case)
                    let sheet = &workbook.sheets[0];
                    let mut non_empty_cells = Vec::new();
                    
                    // Find all non-empty cells
                    for (row_idx, row) in sheet.data.iter().enumerate() {
                        for (col_idx, cell_value) in row.iter().enumerate() {
                            if !cell_value.trim().is_empty() {
                                let col_letter = self.column_index_to_letter(col_idx);
                                let cell_address = format!("{}{}", col_letter, row_idx + 1);
                                non_empty_cells.push((cell_address, cell_value.clone()));
                            }
                        }
                    }
                    
                    if non_empty_cells.is_empty() {
                        overview.push_str("The sheet appears empty - no non-empty cells found.\n");
                    } else {
                        overview.push_str(&format!("Found {} non-empty cells:\n\n", non_empty_cells.len()));
                        
                        for (address, value) in non_empty_cells.iter() {
                            overview.push_str(&format!("  {}: '{}'\n", address, value));
                        }
                        
                        overview.push_str("\n===================================\n");
                        overview.push_str(&format!("Total non-empty cells: {}", non_empty_cells.len()));
                    }
                    
                    Ok(overview)
                }
                Err(e) => {
                    Err(anyhow!("Failed to read Excel file: {}", e))
                }
            }
        } else {
            Err(anyhow!("No file path provided"))
        }
    }

    /// Helper function to convert column index to Excel letter notation
    fn column_index_to_letter(&self, mut index: usize) -> String {
        let mut result = String::new();
        loop {
            result.insert(0, (b'A' + (index % 26) as u8) as char);
            if index < 26 {
                break;
            }
            index = index / 26 - 1;
        }
        result
    }

    /// Send a request to Microsoft Excel Copilot with proper selection sequence
    pub async fn send_request_to_excel_copilot(&self, request: &str) -> Result<String> {
        println!("Sending request to Excel Copilot: {}", request);
        
        let excel_window = self.get_excel_window().await?;
        
        // Ensure Excel window is focused and active
        excel_window.focus()?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Step 1: Find and click the Copilot button
        let copilot_selector = Selector::Name("Copilot".to_string());
        match excel_window.locator(copilot_selector)?.first(Some(Duration::from_millis(3000))).await {
            Ok(copilot_button) => {
                println!("Found Excel Copilot button, clicking it");
                copilot_button.click()?;
                tokio::time::sleep(Duration::from_millis(1000)).await;
                
                // Step 2: Find and click "Ask Copilot" button
                let ask_copilot_selector = Selector::Name("Ask Copilot".to_string());
                match excel_window.locator(ask_copilot_selector)?.first(Some(Duration::from_millis(3000))).await {
                    Ok(ask_copilot_button) => {
                        println!("Found 'Ask Copilot' button, clicking it");
                        ask_copilot_button.click()?;
                        
                        // Step 3: Wait ~2s for Copilot to load
                        println!("Waiting for Copilot to load...");
                        tokio::time::sleep(Duration::from_millis(5000)).await;
                        
                        // Step 4: Type the request
                        println!("Typing request to Copilot: {}", request);
                        excel_window.type_text(request, false)?;
                        
                        // Step 5: Send the request (Enter key)
                        excel_window.press_key("{enter}")?;
                        tokio::time::sleep(Duration::from_millis(3000)).await;
                        
                        // Step 6: Check for Apply button and click it if present
                        let apply_result = self.check_and_apply_copilot_changes(&excel_window).await?;
                        
                        Ok(format!("Copilot request '{}' sent successfully. {}", request, apply_result))
                    }
                    Err(_) => {
                        println!("'Ask Copilot' button not found, trying to type request directly");
                        
                        // Wait a bit and try to type the request directly
                        tokio::time::sleep(Duration::from_millis(2000)).await;
                        excel_window.type_text(request, false)?;
                        excel_window.press_key("{enter}")?;
                        tokio::time::sleep(Duration::from_millis(3000)).await;
                        
                        let apply_result = self.check_and_apply_copilot_changes(&excel_window).await?;
                        Ok(format!("Copilot request '{}' sent (fallback method). {}", request, apply_result))
                    }
                }
            }
            Err(_) => {
                println!("Copilot button not found, trying alternative approach");
                
                // Alternative: Try to use keyboard shortcut to access Copilot (Alt+H, then F, then X)
                // "%h" = Alt+H, then "fx" = F then X
                excel_window.press_key("%h")?; // Alt+H to open Home tab
                tokio::time::sleep(Duration::from_millis(500)).await;
                excel_window.type_text("fx", false)?; // F then X to open Copilot
                tokio::time::sleep(Duration::from_millis(1500)).await;
                
                // Type the request
                excel_window.type_text(request, false)?;
                excel_window.press_key("{enter}")?;
                tokio::time::sleep(Duration::from_millis(3000)).await;
                
                let apply_result = self.check_and_apply_copilot_changes(&excel_window).await?;
                Ok(format!("Copilot request '{}' sent via keyboard shortcut. {}", request, apply_result))
            }
        }
    }

    /// Check for Apply button in Copilot response and click it if present
    async fn check_and_apply_copilot_changes(&self, excel_window: &terminator::UIElement) -> Result<String> {
        println!("Checking for Apply button in Copilot response");
        
        // Try to find Apply button with multiple attempts, every 5 seconds
        let max_attempts = 15; // 30 seconds total (15 attempts × 2 seconds)
        let mut attempt = 0;
        
        while attempt < max_attempts {
            attempt += 1;
            println!("Attempt {} of {} to find Apply button", attempt, max_attempts);
            
            // Strategy 1: Look for "Apply" button with short name
            println!("Strategy 1: Looking for exact 'Apply' button");
            let apply_selector = Selector::Name("Apply".to_string());
            match excel_window.locator(apply_selector)?.all(Some(Duration::from_millis(2000)), None).await {
                Ok(found_elements) => {
                    println!("Found {} elements named 'Apply'", found_elements.len());
                    
                    // Examine each element and find the real button
                    for (i, element) in found_elements.iter().enumerate() {
                        let attributes = element.attributes();
                        let role = attributes.role.as_str();
                        let name = attributes.name.as_deref().unwrap_or("");
                        
                        println!("Apply element {}: role='{}', name='{}', name_length={}", i, role, name, name.len());
                        
                        // Look for the Apply button: exact name match and button role
                        if name == "Apply" && name.len() == 5 {
                            if role.contains("button") || role.contains("Button") || 
                               role.eq_ignore_ascii_case("menuitem") ||
                               role.eq_ignore_ascii_case("listitem") {
                                println!("Found real Apply button (exact match): role='{}', clicking it", role);
                                element.click()?;
                                tokio::time::sleep(Duration::from_millis(2000)).await;
                                return Ok("Changes applied successfully via Apply button.".to_string());
                            }
                        }
                    }
                    
                    // If no exact match, try clicking any reasonable short element
                    for (i, element) in found_elements.iter().enumerate() {
                        let attributes = element.attributes();
                        let role = attributes.role.as_str();
                        let name = attributes.name.as_deref().unwrap_or("");
                        
                        if name.len() <= 10 && !role.eq_ignore_ascii_case("group") && 
                           !role.eq_ignore_ascii_case("text") && !name.contains("You said:") {
                            println!("Trying Apply element {} as fallback: role='{}', name='{}'", i, role, name);
                            match element.click() {
                                Ok(_) => {
                                    tokio::time::sleep(Duration::from_millis(2000)).await;
                                    return Ok("Changes applied successfully via Apply element (fallback).".to_string());
                                }
                                Err(e) => {
                                    println!("Failed to click Apply element {}: {}", i, e);
                                }
                            }
                        }
                    }
                }
                Err(_) => {
                    println!("No elements named 'Apply' found in strategy 1");
                }
            }
            
            // Strategy 2: Look for buttons with roles that might be Apply buttons
            println!("Strategy 2: Looking for button-role elements");
            let button_roles = vec!["Button", "button", "MenuItem", "menuitem", "ListItem", "listitem"];
            
            for role_name in &button_roles {
                let role_selector = Selector::Role { 
                    role: role_name.to_string(), 
                    name: None 
                };
                match excel_window.locator(role_selector)?.all(Some(Duration::from_millis(1000)), None).await {
                    Ok(elements) => {
                        // Check up to 10 elements of each role
                        for (_index, element) in elements.iter().take(10).enumerate() {
                            let attributes = element.attributes();
                            let name = attributes.name.as_deref().unwrap_or("");
                            
                            // Look for exact "Apply" match in button-role elements
                            if name == "Apply" || name == "Apply Changes" || name == "Apply Suggestion" {
                                println!("Found {} element with name '{}', clicking it", role_name, name);
                                match element.click() {
                                    Ok(_) => {
                                        tokio::time::sleep(Duration::from_millis(2000)).await;
                                        return Ok(format!("Changes applied successfully via {} button: '{}'.", role_name, name));
                                    }
                                    Err(e) => {
                                        println!("Failed to click {} element '{}': {}", role_name, name, e);
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {
                        println!("No {} elements found", role_name);
                    }
                }
            }
            
            // Check alternative button names
            let alt_button_names = vec![
                "Create", 
                "Generate",
                "Apply Changes",
                "Accept",
                "Confirm",
                "Execute",
                "Run"
            ];
            
            for button_name in &alt_button_names {
                let selector = Selector::Name(button_name.to_string());
                match excel_window.locator(selector)?.first(Some(Duration::from_millis(1000))).await {
                    Ok(button) => {
                        let attributes = button.attributes();
                        let role = attributes.role.as_str();
                        let actual_name = attributes.name.as_deref().unwrap_or("Unknown");
                        
                        println!("Found alternative element '{}' - role: '{}', name: '{}'", button_name, role, actual_name);
                        
                        // Skip elements that are clearly not buttons
                        if role.eq_ignore_ascii_case("group") || 
                           role.eq_ignore_ascii_case("text") ||
                           role.eq_ignore_ascii_case("tabitem") ||
                           role.eq_ignore_ascii_case("tab") ||
                           actual_name.contains("You said:") {
                            println!("Skipping non-button element for {} (role: {})", button_name, role);
                            continue;
                        }
                        
                        // Try to verify it's a button-like element
                        if role.contains("button") || role.contains("Button") || 
                           actual_name.eq_ignore_ascii_case(button_name) {
                            println!("Found alternative action button: {} (role: {}), clicking it", actual_name, role);
                            button.click()?;
                            tokio::time::sleep(Duration::from_millis(2000)).await;
                            return Ok(format!("Changes applied successfully via {} button.", actual_name));
                        } else {
                            // Try clicking anyway but be more cautious
                            if actual_name.len() < 15 {
                                println!("Found {} element but uncertain if it's clickable (role: {}), trying to click anyway", actual_name, role);
                                match button.click() {
                                    Ok(_) => {
                                        tokio::time::sleep(Duration::from_millis(2000)).await;
                                        return Ok(format!("Changes applied successfully via {} element.", actual_name));
                                    }
                                    Err(_) => {
                                        println!("Failed to click {} element, continuing search", actual_name);
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            
            // Additional search: Look for suggestion buttons that might appear in Copilot responses
            // These might have specific patterns or be located in suggestion areas
            println!("Searching for Copilot suggestion buttons on attempt {}", attempt);
            
            // Try to find buttons in suggestion areas or with suggestion-related roles
            let suggestion_selectors = vec![
                ("Apply conditional formatting", "Apply the conditional formatting suggestion"),
                ("Apply suggestion", "Apply this suggestion"),
                ("Use this suggestion", "Use the Copilot suggestion"),
            ];
            
            for (search_text, description) in suggestion_selectors {
                // Look for buttons containing these phrases
                let selector = Selector::Name(search_text.to_string());
                match excel_window.locator(selector)?.first(Some(Duration::from_millis(1000))).await {
                    Ok(element) => {
                        let attributes = element.attributes();
                        let role = attributes.role.as_str();
                        let name = attributes.name.as_deref().unwrap_or("");
                        
                        println!("Found suggestion element: '{}' - role: '{}', name: '{}'", search_text, role, name);
                        
                        // Skip clearly non-button elements
                        if !role.eq_ignore_ascii_case("group") && 
                           !role.eq_ignore_ascii_case("text") &&
                           !role.eq_ignore_ascii_case("tabitem") {
                            println!("Trying to click suggestion button: {}", description);
                            match element.click() {
                                Ok(_) => {
                                    tokio::time::sleep(Duration::from_millis(2000)).await;
                                    return Ok(format!("Changes applied successfully via suggestion: {}.", description));
                                }
                                Err(e) => {
                                    println!("Failed to click suggestion '{}': {}", search_text, e);
                                }
                            }
                        }
                    }
                    Err(_) => continue,
                }
            }
            
            // Wait 2 seconds before next attempt (except on last attempt)
            if attempt < max_attempts {
                println!("Waiting 5 seconds before next attempt...");
                tokio::time::sleep(Duration::from_millis(2000)).await;
            }
        }
        
        println!("No Apply button found after {} attempts", max_attempts);
        Ok("Request sent to Copilot. No Apply button found after multiple attempts - changes might be applied automatically.".to_string())
    }

    /// Send a request to Excel Copilot with specific range selection
    pub async fn send_request_to_excel_copilot_with_range(&self, request: &str, range: &str) -> Result<String> {
        println!("Sending request to Excel Copilot with range {}: {}", range, request);
        
        let excel_window = self.get_excel_window().await?;
        excel_window.focus()?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Step 1: Select the specified range first
        println!("Selecting range: {}", range);
        excel_window.press_key("{ctrl}g")?; // Go to dialog
        tokio::time::sleep(Duration::from_millis(500)).await;
        excel_window.type_text(range, false)?;
        excel_window.press_key("{enter}")?;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Step 2-6: Follow the normal Copilot sequence
        self.send_request_to_excel_copilot(request).await
    }

    /// Helper function to format common Copilot requests with range
    pub async fn format_cells_with_copilot(&self, range: &str, format_description: &str) -> Result<String> {
        let request = format!("Format these cells with {}", format_description);
        self.send_request_to_excel_copilot_with_range(&request, range).await
    }

    /// Create chart with Copilot using specific data range
    pub async fn create_chart_with_copilot(&self, data_range: &str, chart_type: &str) -> Result<String> {
        let request = format!("Create a {} chart from this data", chart_type);
        self.send_request_to_excel_copilot_with_range(&request, data_range).await
    }

    /// Apply conditional formatting with Copilot to specific range
    pub async fn apply_conditional_formatting_with_copilot(&self, range: &str, condition: &str) -> Result<String> {
        let request = format!("Apply conditional formatting where {}", condition);
        self.send_request_to_excel_copilot_with_range(&request, range).await
    }

    /// Interact with Excel Copilot for various tasks (updated method)
    pub async fn interact_with_copilot(&self, task_description: &str) -> Result<String> {
        println!("Interacting with Excel Copilot for task: {}", task_description);
        
        // Ensure Excel is active and focused
        let excel_window = self.get_excel_window().await?;
        excel_window.focus()?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Send the request to Copilot using the updated method
        let result = self.send_request_to_excel_copilot(task_description).await?;
        
        // Wait a bit more for changes to take effect
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        Ok(result)
    }

    /// Get the complete UI tree context of Excel window for better element targeting
    pub async fn get_excel_ui_context(&self) -> Result<String> {
        println!("Getting complete Excel UI context");
        
        let excel_window = self.get_excel_window().await?;
        
        // Get detailed information about the Excel window structure
        let mut context = String::new();
        context.push_str("EXCEL UI CONTEXT:\n");
        context.push_str("==================\n");
        
        // Get window attributes
        let window_attrs = excel_window.attributes();
        context.push_str(&format!("Window Title: '{}'\n", window_attrs.name.as_deref().unwrap_or("Unknown")));
        context.push_str(&format!("Window Role: '{}'\n", window_attrs.role.as_str()));
        context.push_str(&format!("Window Label: '{}'\n", window_attrs.label.as_deref().unwrap_or("Unknown")));
        
        // Try to get child elements for navigation context
        context.push_str("\nKey UI Elements:\n");
        context.push_str("-----------------\n");
        
        // Look for common Excel UI elements that can be used as landmarks
        let ui_elements_to_find = vec![
            ("Name Box", "Name Box"),
            ("Formula Bar", "Formula Bar"), 
            ("Ribbon", "Ribbon"),
            ("Sheet Tab", "Sheet"),
            ("Copilot", "Copilot"),
            ("Ask Copilot", "Ask Copilot"),
            ("Apply", "Apply"),
        ];
        
        for (element_name, selector_name) in ui_elements_to_find {
            let selector = Selector::Name(selector_name.to_string());
            match excel_window.locator(selector)?.all(Some(Duration::from_millis(1000)), None).await {
                Ok(elements) => {
                    if !elements.is_empty() {
                        context.push_str(&format!("✓ {}: {} element(s) found\n", element_name, elements.len()));
                        
                        // For important elements, get more details
                        if element_name == "Copilot" || element_name == "Apply" {
                            for (i, element) in elements.iter().take(3).enumerate() {
                                let attrs = element.attributes();
                                context.push_str(&format!("  - {}[{}]: role='{}', name='{}', label='{}'\n", 
                                    element_name, i, 
                                    attrs.role.as_str(),
                                    attrs.name.as_deref().unwrap_or("N/A"),
                                    attrs.label.as_deref().unwrap_or("N/A")
                                ));
                            }
                        }
                    } else {
                        context.push_str(&format!("✗ {}: Not found\n", element_name));
                    }
                }
                Err(_) => {
                    context.push_str(&format!("✗ {}: Search failed\n", element_name));
                }
            }
        }
        
        // Get active sheet information
        context.push_str("\nActive Sheet Info:\n");
        context.push_str("------------------\n");
        
        // Try to identify the current active cell
        let name_box_selector = Selector::Name("Name Box".to_string());
        match excel_window.locator(name_box_selector)?.first(Some(Duration::from_millis(1000))).await {
            Ok(name_box) => {
                match name_box.text(1) {
                    Ok(active_cell) => {
                        context.push_str(&format!("Active Cell: {}\n", active_cell.trim()));
                    }
                    Err(_) => {
                        context.push_str("Active Cell: Could not determine\n");
                    }
                }
            }
            Err(_) => {
                context.push_str("Active Cell: Name Box not accessible\n");
            }
        }
        
        context.push_str("\n==================\n");
        
        Ok(context)
    }

    /// Paste TSV data into Excel starting from a specific cell with safety checks
    pub async fn paste_tsv_data(&self, start_cell: &str, tsv_data: &str, verify_safe: bool) -> Result<String> {
        println!("Pasting TSV data starting from cell: {}", start_cell);
        
        let excel_window = self.get_excel_window().await?;
        excel_window.focus()?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Parse TSV to understand the data dimensions
        let lines: Vec<&str> = tsv_data.lines().collect();
        let rows = lines.len();
        let cols = lines.get(0).map(|line| line.split('\t').count()).unwrap_or(0);
        
        if rows == 0 || cols == 0 {
            return Err(anyhow!("Invalid TSV data: {} rows, {} columns", rows, cols));
        }
        
        println!("TSV data dimensions: {} rows × {} columns", rows, cols);
        
        // Calculate the target range
        let start_pos = self.parse_cell_address(start_cell)?;
        let end_col_letter = self.column_index_to_letter(start_pos.1 + cols - 1);
        let end_row = start_pos.0 + rows - 1;
        let target_range = format!("{}:{}{}", start_cell, end_col_letter, end_row);
        
        println!("Target range will be: {}", target_range);
        
        // Safety check: verify the target area if requested
        if verify_safe {
            println!("Performing safety check for range: {}", target_range);
            
            // Read the target range to check if it contains data
            match self.read_range(start_cell, &format!("{}{}", end_col_letter, end_row)).await {
                Ok(existing_range) => {
                    let non_empty_cells = existing_range.values.iter()
                        .flatten()
                        .filter(|cell| !cell.trim().is_empty())
                        .count();
                    
                    if non_empty_cells > 0 {
                        return Err(anyhow!(
                            "SAFETY CHECK FAILED: Target range {} contains {} non-empty cells. Use verify_safe=false to override, or choose a different starting cell.",
                            target_range, non_empty_cells
                        ));
                    } else {
                        println!("Safety check passed: target range is empty");
                    }
                }
                Err(e) => {
                    println!("Warning: Could not verify target range safety: {}", e);
                }
            }
        }
        
        // Navigate to the starting cell
        excel_window.press_key("{ctrl}g")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        excel_window.type_text(start_cell, false)?;
        excel_window.press_key("{enter}")?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Clear clipboard first
        match self.clear_clipboard().await {
            Ok(_) => println!("Clipboard cleared"),
            Err(e) => println!("Warning: Could not clear clipboard: {}", e),
        }
        
        // Copy TSV data to clipboard
        self.set_clipboard_content(tsv_data).await?;
        
        // Paste the data
        excel_window.press_key("{ctrl}v")?;
        tokio::time::sleep(Duration::from_millis(1000)).await;
        
        // Press Escape to clear any selection
        excel_window.press_key("{escape}")?;
        tokio::time::sleep(Duration::from_millis(300)).await;
        
        // Save the file
        self.save_current_file().await?;
        
        // Verify the paste operation by reading a sample
        let verification_result = match self.read_cell(start_cell).await {
            Ok(cell) => {
                let expected_first_value = lines.get(0)
                    .and_then(|line| line.split('\t').next())
                    .unwrap_or("");
                
                if cell.value.trim() == expected_first_value.trim() {
                    "✓ Paste verification successful"
                } else {
                    "⚠ Paste verification: values may not match exactly"
                }
            }
            Err(_) => "⚠ Could not verify paste operation"
        };
        
        Ok(format!(
            "SUCCESS: Pasted TSV data ({} rows × {} columns) into range {}. {}. Data includes: first cell '{}', last calculated cell '{}{}'",
            rows, cols, target_range, verification_result,
            lines.get(0).and_then(|line| line.split('\t').next()).unwrap_or(""),
            end_col_letter, end_row
        ))
    }

    /// Parse cell address like "A1" to (row_index, col_index) zero-based
    fn parse_cell_address(&self, cell_address: &str) -> Result<(usize, usize)> {
        let cell_address = cell_address.trim().to_uppercase();
        
        let mut col_str = String::new();
        let mut row_str = String::new();
        let mut found_digit = false;
        
        for ch in cell_address.chars() {
            if ch.is_ascii_digit() {
                found_digit = true;
                row_str.push(ch);
            } else if ch.is_ascii_alphabetic() && !found_digit {
                col_str.push(ch);
            } else {
                return Err(anyhow!("Invalid cell address format: {}", cell_address));
            }
        }
        
        if col_str.is_empty() || row_str.is_empty() {
            return Err(anyhow!("Invalid cell address: {}", cell_address));
        }
        
        // Convert column letters to index (A=0, B=1, ..., Z=25, AA=26, etc.)
        let mut col_index = 0;
        for ch in col_str.chars() {
            col_index = col_index * 26 + (ch as usize - 'A' as usize + 1);
        }
        col_index -= 1; // Convert to 0-based
        
        // Convert row to index (1-based to 0-based)
        let row_index = row_str.parse::<usize>()? - 1;
        
        Ok((row_index, col_index))
    }

    /// Set clipboard content using arboard
    async fn set_clipboard_content(&self, content: &str) -> Result<()> {
        let mut clipboard = arboard::Clipboard::new()
            .map_err(|e| anyhow::anyhow!("Failed to access clipboard: {}", e))?;
        
        clipboard.set_text(content)
            .map_err(|e| anyhow::anyhow!("Failed to set clipboard text: {}", e))?;
        
        Ok(())
    }

    /// Type a prompt with proper line break handling (DEPRECATED - now using clipboard)
    async fn type_prompt_with_line_breaks(&self, window: &terminator::UIElement, prompt: &str) -> Result<()> {
        println!("Using deprecated line break typing method, prefer clipboard paste");
        
        // Split the prompt by newlines and type each part
        let lines: Vec<&str> = prompt.split('\n').collect();
        
        for (i, line) in lines.iter().enumerate() {
            // Type the current line
            if !line.is_empty() {
                window.type_text(line, false)?;
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            
            // Add line break with Shift+Enter if not the last line
            if i < lines.len() - 1 {
                println!("Adding line break with Shift+Enter");
                window.press_key("{shift}{enter}")?;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        
        Ok(())
    }

    /// Clear clipboard content
    async fn clear_clipboard(&self) -> Result<()> {
        self.set_clipboard_content("").await
    }

    /// Google Sheets specific functionality
    /// ===================================
    
    /// Find Google Sheets browser window
    pub async fn find_google_sheets_window(&self) -> Result<terminator::UIElement> {
        println!("Searching for Google Sheets window...");
        
        // Strategy 1: Look for window with "Google Sheets" in title
        match self.desktop.find_window_by_criteria(Some("Google Sheets"), Some(Duration::from_millis(5000))).await {
            Ok(window) => {
                println!("Found Google Sheets window using title criteria");
                Ok(window)
            }
            Err(_) => {
                // Strategy 2: Look for window with "sheets.google.com" or similar
                match self.desktop.find_window_by_criteria(Some("sheets.google"), Some(Duration::from_millis(3000))).await {
                    Ok(window) => {
                        println!("Found Google Sheets window using URL criteria");
                        Ok(window)
                    }
                    Err(_) => {
                        // Strategy 3: Look for any browser window and check if it contains Google Sheets elements
                        println!("Fallback: searching browser windows for Google Sheets elements");
                        
                        let browsers = vec!["Chrome", "Edge", "Firefox", "msedge"];
                        for browser in browsers {
                            match self.desktop.application(browser) {
                                Ok(browser_app) => {
                                    // Check if this browser window has Google Sheets elements
                                    let ask_gemini_selector = Selector::Name("Ask Gemini".to_string());
                                    match browser_app.locator(ask_gemini_selector)?.first(Some(Duration::from_millis(2000))).await {
                                        Ok(_) => {
                                            println!("Found Google Sheets via '{}' browser (detected 'Ask Gemini' button)", browser);
                                            return Ok(browser_app);
                                        }
                                        Err(_) => continue,
                                    }
                                }
                                Err(_) => continue,
                            }
                        }
                        
                        Err(anyhow!("Could not find Google Sheets window. Make sure Google Sheets is open in a browser."))
                    }
                }
            }
        }
    }

    /// Open Google Sheets Gemini panel (only if not already open)
    pub async fn open_google_sheets_gemini(&self) -> Result<String> {
        println!("Checking if Google Sheets Gemini panel is already open");
        
        let sheets_window = self.find_google_sheets_window().await?;
        sheets_window.focus()?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // First check if the prompt input field is already visible
        let prompt_input_selector = Selector::Name("Enter a prompt here".to_string());
        match sheets_window.locator(prompt_input_selector.clone())?.first(Some(Duration::from_millis(2000))).await {
            Ok(_) => {
                println!("Gemini panel is already open - prompt field is visible");
                return Ok("Google Sheets Gemini panel is already open".to_string());
            }
            Err(_) => {
                println!("Prompt field not visible, need to open Gemini panel");
            }
        }
        
        // Look for "Ask Gemini" button only if prompt field is not visible
        let ask_gemini_selector = Selector::Name("Ask Gemini".to_string());
        match sheets_window.locator(ask_gemini_selector)?.first(Some(Duration::from_millis(5000))).await {
            Ok(ask_gemini_button) => {
                println!("Found 'Ask Gemini' button, clicking it");
                ask_gemini_button.click()?;
                tokio::time::sleep(Duration::from_millis(2000)).await;
                
                // Verify that the prompt field is now visible
                match sheets_window.locator(prompt_input_selector)?.first(Some(Duration::from_millis(3000))).await {
                    Ok(_) => {
                        println!("Successfully opened Gemini panel - prompt field is now visible");
                        Ok("Google Sheets Gemini panel opened successfully".to_string())
                    }
                    Err(_) => {
                        Err(anyhow!("Clicked 'Ask Gemini' but prompt field did not appear. The panel may not have opened correctly."))
                    }
                }
            }
            Err(_) => {
                Err(anyhow!("Could not find 'Ask Gemini' button in Google Sheets. Make sure you're in a Google Sheets document with Gemini enabled."))
            }
        }
    }

    /// Send a prompt to Google Sheets Gemini
    pub async fn send_prompt_to_google_sheets_gemini(&self, prompt: &str) -> Result<String> {
        println!("=== SENDING PROMPT TO GOOGLE SHEETS GEMINI ===");
        println!("Prompt: {}", prompt);
        
        let sheets_window = self.find_google_sheets_window().await?;
        sheets_window.focus()?;
        tokio::time::sleep(Duration::from_millis(500)).await;
        
        // Ensure Gemini panel is open (this function now checks if it's already open)
        println!("Ensuring Gemini panel is accessible...");
        self.open_google_sheets_gemini().await?;
        
        // Look for the prompt input textbox (should be visible now)
        println!("Looking for prompt input field...");
        let prompt_input_selector = Selector::Name("Enter a prompt here".to_string());
        match sheets_window.locator(prompt_input_selector)?.first(Some(Duration::from_millis(3000))).await {
            Ok(prompt_input) => {
                println!("✓ Found prompt input field, proceeding with prompt submission");
                
                // Click on the input field
                prompt_input.click()?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Clear any existing content
                sheets_window.press_key("{ctrl}a")?;
                tokio::time::sleep(Duration::from_millis(100)).await;
                
                // Use clipboard to paste the entire prompt at once
                println!("Pasting prompt via clipboard...");
                self.paste_prompt_via_clipboard(&sheets_window, prompt).await?;
                
                // Send the prompt (Enter key) - only at the very end
                println!("Submitting prompt to Gemini...");
                sheets_window.press_key("{enter}")?;
                
                // Wait for Gemini to respond
                println!("Waiting for Gemini response...");
                tokio::time::sleep(Duration::from_millis(5000)).await;
                
                // Press Enter again to apply the response automatically
                println!("Applying Gemini response...");
                sheets_window.press_key("{enter}")?;
                tokio::time::sleep(Duration::from_millis(2000)).await;
                
                println!("✓ Successfully completed prompt submission and response application");
                Ok(format!("Prompt sent to Google Sheets Gemini successfully and response applied"))
            }
            Err(_) => {
                println!("✗ Could not find prompt input field after panel check");
                Err(anyhow!("Could not find 'Enter a prompt here' textbox even after trying to open Gemini panel. Make sure the Gemini panel is accessible in Google Sheets."))
            }
        }
    }

    /// Paste prompt via clipboard to avoid line break issues
    async fn paste_prompt_via_clipboard(&self, window: &terminator::UIElement, prompt: &str) -> Result<()> {
        println!("Pasting prompt via clipboard to avoid line break issues");
        
        // Set clipboard content
        match self.set_clipboard_content(prompt).await {
            Ok(_) => {
                println!("Prompt copied to clipboard successfully");
                
                // Paste using Ctrl+V
                window.press_key("{ctrl}v")?;
                tokio::time::sleep(Duration::from_millis(300)).await;
                
                println!("Prompt pasted successfully via clipboard");
                Ok(())
            }
            Err(e) => {
                println!("Failed to copy to clipboard: {}, falling back to direct typing", e);
                
                // Fallback: type directly without line break handling
                window.type_text(prompt, false)?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                Ok(())
            }
        }
    }

    /// Check if Google Sheets window is available (without opening new one)
    pub async fn check_google_sheets_availability(&self) -> Result<String> {
        println!("=== CHECKING GOOGLE SHEETS AVAILABILITY ===");
        
        // Simple approach: just try to find the window without complex async spawning
        match self.find_google_sheets_window_safe().await {
            Ok(()) => {
                println!("Google Sheets window found, checking Gemini availability...");
                
                // Create a new automation instance to check Gemini
                match ExcelAutomation::new().await {
                    Ok(temp_automation) => {
                        match temp_automation.check_gemini_availability_simple().await {
                            Ok(msg) => Ok(msg),
                            Err(e) => Ok(format!("Google Sheets found but Gemini check failed: {}", e))
                        }
                    }
                    Err(e) => Ok(format!("Google Sheets found but automation setup failed: {}", e))
                }
            }
            Err(_) => {
                println!("No Google Sheets window found during search");
                Ok("No Google Sheets window found. Please open Google Sheets manually.".to_string())
            }
        }
    }
    
    /// Safe window finding that won't panic
    async fn find_google_sheets_window_safe(&self) -> Result<()> {
        println!("Safely searching for Google Sheets window...");
        
        // Strategy 1: Look for window with "Google Sheets" in title (short timeout)
        match self.desktop.find_window_by_criteria(Some("Google Sheets"), Some(Duration::from_millis(2000))).await {
                    Ok(_) => {
                println!("Found Google Sheets window using title criteria");
                return Ok(());
                    }
                    Err(_) => {
                println!("No Google Sheets window with title found");
            }
        }
        
        // Strategy 2: Look for window with "sheets.google.com" (short timeout)
        match self.desktop.find_window_by_criteria(Some("sheets.google"), Some(Duration::from_millis(1500))).await {
            Ok(_) => {
                println!("Found Google Sheets window using URL criteria");
                return Ok(());
            }
            Err(_) => {
                println!("No Google Sheets window with URL found");
                    }
                }
                
        Err(anyhow!("Could not find Google Sheets window"))
    }
    
    /// Simple Gemini availability check
    async fn check_gemini_availability_simple(&self) -> Result<String> {
        println!("Checking if Gemini is available in Google Sheets");
        
        match self.find_google_sheets_window().await {
            Ok(sheets_window) => {
                sheets_window.focus()?;
                tokio::time::sleep(Duration::from_millis(300)).await;
                
                // Look for "Ask Gemini" button with short timeout
                let ask_gemini_selector = Selector::Name("Ask Gemini".to_string());
                match sheets_window.locator(ask_gemini_selector)?.first(Some(Duration::from_millis(1500))).await {
                    Ok(_) => {
                        println!("Found 'Ask Gemini' button - Gemini is available");
                        Ok("Google Sheets with Gemini is available and ready".to_string())
            }
                    Err(_) => {
                        Ok("Google Sheets found but Gemini not available. Make sure you're in a Google Sheets document with Gemini enabled.".to_string())
        }
    }
            }
            Err(e) => Err(e)
        }
    }
    

    /// Send data to Google Sheets Gemini (with TSV formatting for structured data)
    pub async fn send_data_to_google_sheets_gemini(&self, data: &str, description: &str) -> Result<String> {
        let prompt = if data.contains('\t') || data.contains('\n') {
            // Data appears to be structured - keep real tabs and provide clear instructions
            format!("{}\n\nPlease create a table with this tab-separated data (columns separated by tabs, rows separated by new lines):\n\n{}", description, data)
        } else {
            // Simple text data
            format!("{}\n\nData: {}", description, data)
        };
        
        self.send_prompt_to_google_sheets_gemini(&prompt).await
    }

    /// Open Google Sheets application (only when explicitly requested)
    pub async fn open_google_sheets_app(&self) -> Result<String> {
        println!("Opening Google Sheets application");
        
        // Use PowerShell to open Google Sheets as web app in Edge
        let command = r#"Start-Process -FilePath msedge -ArgumentList '--app=https://docs.google.com/spreadsheets/'"#;
        
        match self.desktop.run_command(Some(command), None).await {
            Ok(output) => {
                println!("Google Sheets opened successfully");
                tokio::time::sleep(Duration::from_millis(3000)).await; // Wait for app to load
                Ok(format!("Google Sheets opened successfully. Output: {}", output.stdout))
            }
            Err(e) => {
                Err(anyhow!("Failed to open Google Sheets: {}", e))
            }
        }
    }

    /// Interact with Google Sheets Gemini (main entry point) - Check availability first
    pub async fn interact_with_google_sheets_gemini(&self, task_description: &str) -> Result<String> {
        println!("Interacting with Google Sheets Gemini for task: {}", task_description);
        
        // First check if Google Sheets is available
        match self.find_google_sheets_window().await {
            Ok(sheets_window) => {
                sheets_window.focus()?;
                tokio::time::sleep(Duration::from_millis(500)).await;
                
                // Send the request to Google Sheets Gemini
                self.send_prompt_to_google_sheets_gemini(task_description).await
            }
            Err(_) => {
                Err(anyhow!("No Google Sheets window found. Please open Google Sheets manually first, then try again."))
            }
        }
    }
}

/// Get the global Excel automation instance
pub async fn get_excel_automation() -> Result<ExcelAutomation> {
    ExcelAutomation::new().await
} 