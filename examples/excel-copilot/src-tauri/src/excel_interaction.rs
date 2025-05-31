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
        let desktop = Desktop::new(false, false).await?;
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
}

/// Get the global Excel automation instance
pub async fn get_excel_automation() -> Result<ExcelAutomation> {
    ExcelAutomation::new().await
} 