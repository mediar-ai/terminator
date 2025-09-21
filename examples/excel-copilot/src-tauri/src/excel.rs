use anyhow::{Result, anyhow};
use calamine::{Reader, Xlsx, open_workbook};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use rust_xlsxwriter::Workbook;

/// Represents a cell position in Excel format (e.g., A1, B2, etc.)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellPosition {
    pub column: usize,
    pub row: usize,
}

/// Represents a range of cells (e.g., A1:C10)
#[derive(Debug, Clone)]
pub struct CellRange {
    pub start: CellPosition,
    pub end: CellPosition,
}

/// Individual Excel sheet with enhanced functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelSheet {
    pub name: String,
    pub data: Vec<Vec<String>>,
}

/// Main Excel workbook with enhanced functionality
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExcelWorkbook {
    pub file_path: Option<String>,
    pub sheets: Vec<ExcelSheet>,
}

/// Cell range utilities and helper functions
impl CellPosition {
    /// Create a new cell position from column and row indices (0-based)
    pub fn new(column: usize, row: usize) -> Self {
        Self { column, row }
    }

    /// Parse Excel cell notation (e.g., "A1", "B2") to CellPosition
    pub fn from_excel_notation(notation: &str) -> Result<Self> {
        let notation = notation.trim().to_uppercase();
        if notation.is_empty() {
            return Err(anyhow!("Empty cell notation"));
        }

        let mut column = 0;
        let mut row_str = String::new();
        let mut found_digit = false;

        for ch in notation.chars() {
            if ch.is_ascii_digit() {
                found_digit = true;
                row_str.push(ch);
            } else if ch.is_ascii_alphabetic() && !found_digit {
                column = column * 26 + (ch as usize - 'A' as usize + 1);
            } else {
                return Err(anyhow!("Invalid cell notation: {}", notation));
            }
        }

        if row_str.is_empty() {
            return Err(anyhow!("No row number found in notation: {}", notation));
        }

        let row = row_str.parse::<usize>()
            .map_err(|_| anyhow!("Invalid row number: {}", row_str))?;
        
        if row == 0 {
            return Err(anyhow!("Row numbers start from 1"));
        }

        Ok(Self::new(column - 1, row - 1)) // Convert to 0-based indexing
    }

    /// Convert to Excel notation (e.g., A1, B2)
    pub fn to_excel_notation(&self) -> String {
        let mut column_str = String::new();
        let mut col = self.column + 1; // Convert to 1-based

        while col > 0 {
            col -= 1;
            column_str.insert(0, (b'A' + (col % 26) as u8) as char);
            col /= 26;
        }

        format!("{}{}", column_str, self.row + 1)
    }
}

impl CellRange {
    /// Create a new cell range
    pub fn new(start: CellPosition, end: CellPosition) -> Self {
        Self { start, end }
    }

    /// Parse Excel range notation (e.g., "A1:C10") to CellRange
    pub fn from_excel_notation(notation: &str) -> Result<Self> {
        let notation = notation.trim().to_uppercase();
        
        if let Some(colon_pos) = notation.find(':') {
            let start_str = &notation[..colon_pos];
            let end_str = &notation[colon_pos + 1..];
            
            let start = CellPosition::from_excel_notation(start_str)?;
            let end = CellPosition::from_excel_notation(end_str)?;
            
            Ok(Self::new(start, end))
        } else {
            // Single cell range
            let pos = CellPosition::from_excel_notation(&notation)?;
            Ok(Self::new(pos.clone(), pos))
        }
    }

    /// Convert to Excel notation (e.g., A1:C10)
    pub fn to_excel_notation(&self) -> String {
        if self.start == self.end {
            self.start.to_excel_notation()
        } else {
            format!("{}:{}", self.start.to_excel_notation(), self.end.to_excel_notation())
        }
    }

    /// Check if the range contains a specific position
    pub fn contains(&self, pos: &CellPosition) -> bool {
        pos.column >= self.start.column && pos.column <= self.end.column &&
        pos.row >= self.start.row && pos.row <= self.end.row
    }

    /// Get all positions within this range
    pub fn positions(&self) -> Vec<CellPosition> {
        let mut positions = Vec::new();
        for row in self.start.row..=self.end.row {
            for col in self.start.column..=self.end.column {
                positions.push(CellPosition::new(col, row));
            }
        }
        positions
    }
}

impl ExcelSheet {
    /// Create a new empty sheet
    pub fn new(name: String) -> Self {
        Self {
            name,
            data: Vec::new(),
        }
    }

    /// Create a sheet with sample data
    pub fn with_blank_data(name: String) -> Self {
        Self {
            name,
            data: Vec::new(),
        }
    }

    /// Get cell value at specific position
    pub fn get_cell(&self, pos: &CellPosition) -> Option<&String> {
        self.data.get(pos.row)?.get(pos.column)
    }

    /// Set cell value at specific position
    pub fn set_cell(&mut self, pos: &CellPosition, value: String) {
        // Ensure the data grid is large enough
        while self.data.len() <= pos.row {
            self.data.push(Vec::new());
        }
        
        let row = &mut self.data[pos.row];
        while row.len() <= pos.column {
            row.push(String::new());
        }
        
        row[pos.column] = value;
    }

    /// Get all cells in a range
    pub fn get_range(&self, range: &CellRange) -> Vec<(CellPosition, String)> {
        let mut cells = Vec::new();
        for pos in range.positions() {
            if let Some(value) = self.get_cell(&pos) {
                cells.push((pos, value.clone()));
            }
        }
        cells
    }

    /// Get a specific column (all cells in that column)
    pub fn get_column(&self, column_index: usize) -> Vec<(usize, String)> {
        let mut column_data = Vec::new();
        for (row_index, row) in self.data.iter().enumerate() {
            if let Some(value) = row.get(column_index) {
                column_data.push((row_index, value.clone()));
            }
        }
        column_data
    }

    /// Get a specific row (all cells in that row)
    pub fn get_row(&self, row_index: usize) -> Option<&Vec<String>> {
        self.data.get(row_index)
    }

    /// Display cells in a range with Excel notation
    pub fn display_range(&self, range_notation: &str) -> Result<String> {
        let range = CellRange::from_excel_notation(range_notation)?;
        let cells = self.get_range(&range);
        
        let mut result = format!("Range {} in sheet '{}':\n", range_notation, self.name);
        
        if cells.is_empty() {
            result.push_str("  (empty)\n");
            return Ok(result);
        }

        // Group by rows for better display
        let mut rows: HashMap<usize, Vec<(usize, String)>> = HashMap::new();
        for (pos, value) in cells {
            rows.entry(pos.row).or_insert_with(Vec::new).push((pos.column, value));
        }

        let mut sorted_rows: Vec<_> = rows.into_iter().collect();
        sorted_rows.sort_by_key(|(row, _)| *row);

        for (row_index, mut columns) in sorted_rows {
            columns.sort_by_key(|(col, _)| *col);
            result.push_str(&format!("  Row {}: ", row_index + 1));
            
            for (col_index, value) in columns {
                let pos = CellPosition::new(col_index, row_index);
                result.push_str(&format!("{}={} ", pos.to_excel_notation(), value));
            }
            result.push('\n');
        }

        Ok(result)
    }

    /// Get sheet dimensions (rows, columns)
    pub fn dimensions(&self) -> (usize, usize) {
        let rows = self.data.len();
        let cols = self.data.iter().map(|row| row.len()).max().unwrap_or(0);
        (rows, cols)
    }

    /// Get cell range data as Vec<Vec<String>>
    pub fn get_cell_range(&self, range: &CellRange) -> Result<Vec<Vec<String>>> {
        let start_row = range.start.row;
        let end_row = range.end.row;
        let start_col = range.start.column;
        let end_col = range.end.column;

        let mut result = Vec::new();
        for row_idx in start_row..=end_row {
            let mut row_data = Vec::new();
            for col_idx in start_col..=end_col {
                let pos = CellPosition::new(col_idx, row_idx);
                let value = self.get_cell(&pos).unwrap_or(&String::new()).clone();
                row_data.push(value);
            }
            result.push(row_data);
        }
        Ok(result)
    }

    /// Get column data by column notation (e.g., "A", "B", "C")
    pub fn get_column_data(&self, column_notation: &str) -> Result<Vec<String>> {
        let pos = CellPosition::from_excel_notation(&format!("{}1", column_notation.to_uppercase()))?;
        let column_data = self.get_column(pos.column);
        Ok(column_data.into_iter().map(|(_, value)| value).collect())
    }

    /// Set cell value by position
    pub fn set_cell_value(&mut self, pos: &CellPosition, value: &str) -> Result<()> {
        self.set_cell(pos, value.to_string());
        Ok(())
    }

    /// Get cell value by position
    pub fn get_cell_value(&self, pos: &CellPosition) -> Result<String> {
        Ok(self.get_cell(pos).unwrap_or(&String::new()).clone())
    }
}

/// Helper function to convert DataType to String
fn datatype_to_string<T: std::fmt::Display>(cell: &T) -> String {
    format!("{}", cell)
}

impl ExcelWorkbook {
    /// Create a new empty workbook
    pub fn new() -> Self {
        Self {
            file_path: None,
            sheets: vec![ExcelSheet::with_blank_data("Sheet1".to_string())],
        }
    }

    /// Load workbook from file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let mut workbook: Xlsx<_> = open_workbook(path)
            .map_err(|e| anyhow!("Failed to open Excel file: {}", e))?;

        let mut sheets = Vec::new();
        
        for sheet_name in workbook.sheet_names().to_vec() {
            if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                let mut data = Vec::new();
                
                for row in range.rows() {
                    let row_data: Vec<String> = row.iter()
                        .map(|cell| datatype_to_string(cell))
                        .collect();
                    data.push(row_data);
                }

                sheets.push(ExcelSheet {
                    name: sheet_name.to_string(),
                    data,
                });
            }
        }

        Ok(Self {
            file_path: Some(path.to_string_lossy().to_string()),
            sheets,
        })
    }

    /// Save workbook to Excel file using rust_xlsxwriter
    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let mut workbook = Workbook::new();
        
        for sheet in &self.sheets {
            let worksheet = workbook.add_worksheet()
                .set_name(&sheet.name)
                .map_err(|e| anyhow!("Failed to add worksheet '{}': {}", sheet.name, e))?;

            // Write data to worksheet
            for (row_index, row) in sheet.data.iter().enumerate() {
                for (col_index, value) in row.iter().enumerate() {
                    if !value.is_empty() {
                        // Try to parse as number first, then write as string
                        if let Ok(num) = value.parse::<f64>() {
                            worksheet.write_number(row_index as u32, col_index as u16, num)
                                .map_err(|e| anyhow!("Failed to write number at {}:{}: {}", row_index, col_index, e))?;
                        } else {
                            worksheet.write_string(row_index as u32, col_index as u16, value)
                                .map_err(|e| anyhow!("Failed to write string at {}:{}: {}", row_index, col_index, e))?;
                        }
                    }
                }
            }
        }

        workbook.save(path.as_ref())
            .map_err(|e| anyhow!("Failed to save workbook: {}", e))?;

        Ok(())
    }

    /// Get sheet by name
    pub fn get_sheet(&self, name: &str) -> Option<&ExcelSheet> {
        self.sheets.iter().find(|sheet| sheet.name == name)
    }

    /// Get mutable sheet by name
    pub fn get_sheet_mut(&mut self, name: &str) -> Option<&mut ExcelSheet> {
        self.sheets.iter_mut().find(|sheet| sheet.name == name)
    }

    /// Add a new sheet
    pub fn add_sheet(&mut self, sheet: ExcelSheet) {
        self.sheets.push(sheet);
    }

    /// Display specific range across all sheets
    pub fn display_range_all_sheets(&self, range_notation: &str) -> Result<String> {
        let mut result = format!("Range {} across all sheets:\n\n", range_notation);
        
        for sheet in &self.sheets {
            match sheet.display_range(range_notation) {
                Ok(sheet_display) => result.push_str(&sheet_display),
                Err(e) => result.push_str(&format!("Error in sheet '{}': {}\n", sheet.name, e)),
            }
            result.push('\n');
        }
        
        Ok(result)
    }

    /// Display specific column across all sheets
    pub fn display_column_all_sheets(&self, column_notation: &str) -> Result<String> {
        let pos = CellPosition::from_excel_notation(&format!("{}1", column_notation.to_uppercase()))?;
        let mut result = format!("Column {} across all sheets:\n\n", column_notation.to_uppercase());
        
        for sheet in &self.sheets {
            result.push_str(&format!("Sheet '{}':\n", sheet.name));
            let column_data = sheet.get_column(pos.column);
            
            if column_data.is_empty() {
                result.push_str("  (empty column)\n");
            } else {
                for (row_index, value) in column_data {
                    if !value.is_empty() {
                        let cell_pos = CellPosition::new(pos.column, row_index);
                        result.push_str(&format!("  {} = {}\n", cell_pos.to_excel_notation(), value));
                    }
                }
            }
            result.push('\n');
        }
        
        Ok(result)
    }

    /// Convert to CSV string (for backward compatibility)
    pub fn to_csv_string(&self) -> String {
        let mut result = String::new();
        
        for (sheet_idx, sheet) in self.sheets.iter().enumerate() {
            if sheet_idx > 0 {
                result.push_str("\n\n--- Sheet: ");
                result.push_str(&sheet.name);
                result.push_str(" ---\n");
            }
            
            for row in &sheet.data {
                let csv_row = row.join(",");
                result.push_str(&csv_row);
                result.push('\n');
            }
        }
        
        result
    }

    /// Get content summary with enhanced information
    pub fn get_content_summary(&self) -> String {
        let mut summary = String::new();
        summary.push_str(&format!("Excel workbook with {} sheet(s):\n\n", self.sheets.len()));
        
        for sheet in &self.sheets {
            let (rows, cols) = sheet.dimensions();
            summary.push_str(&format!("Sheet '{}': {} rows, {} columns\n", 
                sheet.name, rows, cols
            ));
            
            // Show first few rows as sample
            for (i, row) in sheet.data.iter().take(5).enumerate() {
                summary.push_str(&format!("  Row {}: {}\n", i + 1, row.join(" | ")));
            }
            
            if sheet.data.len() > 5 {
                summary.push_str(&format!("  ... and {} more rows\n", sheet.data.len() - 5));
            }
            summary.push('\n');
        }
        
        summary
    }

    /// Quick access functions for common operations
    pub fn get_cell_value(&self, sheet_name: &str, cell_notation: &str) -> Result<Option<String>> {
        let sheet = self.get_sheet(sheet_name)
            .ok_or_else(|| anyhow!("Sheet '{}' not found", sheet_name))?;
        
        let pos = CellPosition::from_excel_notation(cell_notation)?;
        Ok(sheet.get_cell(&pos).cloned())
    }

    pub fn set_cell_value(&mut self, sheet_name: &str, cell_notation: &str, value: String) -> Result<()> {
        let sheet = self.get_sheet_mut(sheet_name)
            .ok_or_else(|| anyhow!("Sheet '{}' not found", sheet_name))?;
        
        let pos = CellPosition::from_excel_notation(cell_notation)?;
        sheet.set_cell(&pos, value);
        Ok(())
    }
} 