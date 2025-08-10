use crate::events::{
    ClickEvent, EnhancedUIElement, McpToolStep, TextInputCompletedEvent, WorkflowEvent,
};
use anyhow::Result;
use serde_json::json;
use tracing::{debug, warn};

/// Configuration for MCP conversion behavior
#[derive(Debug, Clone)]
pub struct ConversionConfig {
    /// Whether to include MCP conversion during recording
    pub enable_mcp_conversion: bool,
    /// Whether to detect UI patterns during recording
    pub enable_pattern_detection: bool,
    /// Maximum number of fallback strategies to generate
    pub max_fallback_strategies: usize,
    /// Whether to validate generated sequences during recording
    pub validate_during_recording: bool,
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            enable_mcp_conversion: true,
            enable_pattern_detection: true,
            max_fallback_strategies: 3,
            validate_during_recording: false, // Expensive, off by default
        }
    }
}

/// Result of converting a workflow event to MCP sequences
#[derive(Debug, Clone)]
pub struct ConversionResult {
    /// Primary MCP tool sequence
    pub primary_sequence: Vec<McpToolStep>,
    /// Semantic action description
    pub semantic_action: String,
    /// Alternative sequences as fallbacks
    pub fallback_sequences: Vec<Vec<McpToolStep>>,
    /// Analysis notes for debugging
    pub conversion_notes: Vec<String>,
}

/// Converts workflow events into MCP-compatible tool sequences
#[derive(Clone)]
pub struct McpConverter {
    config: ConversionConfig,
}

impl McpConverter {
    /// Create a new MCP converter with default configuration
    pub fn new() -> Self {
        Self::with_config(ConversionConfig::default())
    }

    /// Create a new MCP converter with custom configuration
    pub fn with_config(config: ConversionConfig) -> Self {
        Self { config }
    }

    /// Convert a workflow event to MCP sequences
    pub async fn convert_event(
        &self,
        event: &WorkflowEvent,
        ui_context: Option<&EnhancedUIElement>,
    ) -> Result<ConversionResult> {
        if !self.config.enable_mcp_conversion {
            return Ok(ConversionResult {
                primary_sequence: vec![],
                semantic_action: "disabled".to_string(),
                fallback_sequences: vec![],
                conversion_notes: vec!["MCP conversion disabled".to_string()],
            });
        }

        debug!("Converting workflow event to MCP sequence: {:?}", event);

        match event {
            WorkflowEvent::TextInputCompleted(text_event) => {
                self.convert_text_input(text_event, ui_context).await
            }
            WorkflowEvent::Click(click_event) => self.convert_click(click_event, ui_context).await,
            WorkflowEvent::ApplicationSwitch(app_event) => {
                self.convert_application_switch(app_event).await
            }
            WorkflowEvent::BrowserTabNavigation(nav_event) => {
                self.convert_browser_navigation(nav_event).await
            }
            // Add other event types as needed
            _ => {
                warn!("MCP conversion not implemented for event type: {:?}", event);
                Ok(ConversionResult {
                    primary_sequence: vec![],
                    semantic_action: "unsupported".to_string(),
                    fallback_sequences: vec![],
                    conversion_notes: vec![
                        "Event type not supported for MCP conversion".to_string()
                    ],
                })
            }
        }
    }

    /// Convert text input event to MCP sequence
    async fn convert_text_input(
        &self,
        event: &TextInputCompletedEvent,
        ui_context: Option<&EnhancedUIElement>,
    ) -> Result<ConversionResult> {
        let mut notes = Vec::new();

        // Analyze input method to determine conversion strategy
        let conversion_strategy = match event.input_method {
            crate::events::TextInputMethod::Suggestion => {
                notes.push("Detected suggestion-based input".to_string());
                self.convert_suggestion_input(event, ui_context).await?
            }
            crate::events::TextInputMethod::Typed => {
                notes.push("Detected typed input".to_string());
                self.convert_typed_input(event, ui_context).await?
            }
            crate::events::TextInputMethod::Pasted => {
                notes.push("Detected pasted input".to_string());
                self.convert_pasted_input(event, ui_context).await?
            }
            _ => {
                notes.push("Using fallback conversion for mixed/unknown input method".to_string());
                self.convert_fallback_input(event, ui_context).await?
            }
        };

        Ok(ConversionResult {
            primary_sequence: conversion_strategy.sequence,
            semantic_action: conversion_strategy.semantic_action,
            fallback_sequences: conversion_strategy.fallbacks,
            conversion_notes: notes,
        })
    }

    /// Convert click event to MCP sequence
    async fn convert_click(
        &self,
        event: &ClickEvent,
        ui_context: Option<&EnhancedUIElement>,
    ) -> Result<ConversionResult> {
        let mut sequence = Vec::new();
        let mut notes = Vec::new();

        // Extract application/window context for scoped selector generation
        let window_context = if let Some(metadata) = &event.metadata.ui_element {
            let app_name = metadata.application_name();
            let window_title = metadata.window_title();

            // FIX: Use window container role, not clicked element role
            let window_role = metadata
                .window()
                .ok()
                .flatten()
                .map(|w| w.role())
                .unwrap_or_else(|| "Window".to_string());

            // Use app_name as the source of window context (recorder stores window titles there)
            let effective_window_title = if !app_name.is_empty() {
                &app_name
            } else {
                &window_title
            };
            let effective_app_name = if app_name.contains("Chrome") {
                "Google Chrome"
            } else if app_name.contains("Firefox") {
                "Firefox"
            } else if app_name.contains("Edge") {
                "Microsoft Edge"
            } else {
                "Application"
            };

            tracing::info!("🔍 MCP Converter - app: '{}', title: '{}', window_role: '{}', clicked_element_role: '{}'", 
                          effective_app_name, effective_window_title, window_role, metadata.role());

            if !effective_window_title.is_empty() && effective_window_title.len() > 3 {
                Some((
                    effective_app_name.to_string(),
                    effective_window_title.to_string(),
                    window_role.to_string(),
                ))
            } else {
                None
            }
        } else {
            None
        };

        // NEW: Check if this is a click-away action to dismiss UI elements
        if self.is_click_away_action(event) {
            tracing::info!("🔄 Detected click-away action - converting to Escape key press");

            // Generate escape key step instead of click
            let escape_step = self.generate_escape_key_step();
            sequence.push(escape_step);

            notes.push(format!("Converted click-away action to Escape key press - detected container click: role='{}', children={}", 
                event.element_role, event.child_text_content.len()));

            tracing::info!("✅ Generated Escape key press for click-away dismissal");

            return Ok(ConversionResult {
                semantic_action: "dismiss_ui".to_string(),
                primary_sequence: sequence,
                fallback_sequences: vec![],
                conversion_notes: notes,
            });
        }

        // Generate scoped selector for the element using >> operator
        let selector = if let Some(context) = ui_context {
            notes.push("Using enhanced UI context for selector generation".to_string());
            context
                .suggested_selectors
                .first()
                .cloned()
                .unwrap_or_else(|| self.generate_scoped_selector(event, &window_context))
        } else {
            notes.push("Using scoped selector generation from event data".to_string());
            self.generate_scoped_selector(event, &window_context)
        };

        // Add note about scoped selector usage
        if window_context.is_some() {
            notes
                .push("Generated scoped selector using >> operator for window context".to_string());
        }

        // Create the click step with 3000ms timeout as requested
        sequence.push(McpToolStep {
            tool_name: "click_element".to_string(),
            arguments: json!({
                "selector": selector,
                "timeout_ms": 3000
            }),
            description: format!(
                "Click '{}' element",
                if !event.element_text.is_empty() {
                    &event.element_text
                } else if !event.child_text_content.is_empty() {
                    &event.child_text_content[0]
                } else {
                    &event.element_role
                }
            ),
            timeout_ms: Some(3000),
            continue_on_error: Some(false),
            delay_ms: Some(200),
        });

        // No fallback sequences as requested
        Ok(ConversionResult {
            primary_sequence: sequence,
            semantic_action: "element_click".to_string(),
            fallback_sequences: vec![], // No fallbacks as requested
            conversion_notes: notes,
        })
    }

    /// Convert application switch event to MCP sequence
    async fn convert_application_switch(
        &self,
        event: &crate::events::ApplicationSwitchEvent,
    ) -> Result<ConversionResult> {
        let mut sequence = Vec::new();
        let mut notes = Vec::new();

        sequence.push(McpToolStep {
            tool_name: "activate_element".to_string(),
            arguments: json!({
                "selector": format!("application|{}", event.to_application)
            }),
            description: format!("Switch to application: {}", event.to_application),
            timeout_ms: Some(3000),
            continue_on_error: Some(false),
            delay_ms: Some(1000),
        });

        notes.push(format!(
            "Application switch method: {:?}",
            event.switch_method
        ));

        Ok(ConversionResult {
            primary_sequence: sequence,
            semantic_action: "application_switch".to_string(),
            fallback_sequences: vec![],
            conversion_notes: notes,
        })
    }

    /// Convert browser navigation event to MCP sequence
    async fn convert_browser_navigation(
        &self,
        event: &crate::events::BrowserTabNavigationEvent,
    ) -> Result<ConversionResult> {
        let mut sequence = Vec::new();
        let mut notes = Vec::new();

        if let Some(url) = &event.to_url {
            sequence.push(McpToolStep {
                tool_name: "open_url".to_string(),
                arguments: json!({
                    "url": url
                }),
                description: format!("Navigate to URL: {}", url),
                timeout_ms: Some(10000),
                continue_on_error: Some(false),
                delay_ms: Some(1000),
            });
            notes.push(format!("Browser navigation to: {}", url));
        }

        Ok(ConversionResult {
            primary_sequence: sequence,
            semantic_action: "browser_navigation".to_string(),
            fallback_sequences: vec![],
            conversion_notes: notes,
        })
    }

    /// Detect if this click is a "click away" action to dismiss UI elements
    fn is_click_away_action(&self, event: &ClickEvent) -> bool {
        // Non-clickable container roles that are typically used for layout, not interaction
        const NON_CLICKABLE_ROLES: &[&str] = &[
            "custom",        // Generic containers
            "document",      // Page content areas
            "group",         // Layout containers
            "main",          // Main content areas
            "section",       // Content sections
            "article",       // Article containers
            "div",           // Generic divs
            "region",        // ARIA regions
            "banner",        // Header areas
            "contentinfo",   // Footer areas
            "complementary", // Sidebar areas
            "generic",       // Generic elements
            "pane",          // Content panes
            "client",        // Client areas
        ];

        // Generic container names that suggest layout rather than interaction
        const GENERIC_NAMES: &[&str] = &[
            "home",
            "main",
            "content",
            "container",
            "page",
            "body",
            "wrapper",
            "layout",
            "section",
            "area",
            "panel",
            "view",
            "canvas",
            "workspace",
        ];

        // Check if role suggests a non-interactive container
        let is_non_clickable_role =
            NON_CLICKABLE_ROLES.contains(&event.element_role.to_lowercase().as_str());

        // Check if element has many children (strong indicator of layout container)
        let has_many_children = event.child_text_content.len() >= 8;

        // Check if element name suggests a generic container
        let has_generic_name = GENERIC_NAMES
            .iter()
            .any(|&name| event.element_text.to_lowercase().contains(name));

        // Future enhancement: Check bounds information to detect large containers
        // let is_large_container = if let Some(metadata) = &event.metadata.ui_element {
        //     // This would require accessing bounds from UIElement
        //     false
        // } else {
        //     false
        // };

        // Classify as click-away if it's a non-clickable role AND has container characteristics
        is_non_clickable_role && (has_many_children || has_generic_name)
    }

    /// Generate escape key press for dismissing UI elements
    fn generate_escape_key_step(&self) -> McpToolStep {
        McpToolStep {
            tool_name: "key_press".to_string(),
            arguments: json!({
                "key": "Escape",
                "timeout_ms": 1000
            }),
            description: "Press Escape to dismiss dropdown/modal/overlay".to_string(),
            timeout_ms: Some(1000),
            continue_on_error: Some(false),
            delay_ms: Some(100),
        }
    }

    /// Generate primary selector for element clicks - prefers child text when more specific
    #[allow(dead_code)] // TODO: Will be used for enhanced selector generation
    fn generate_primary_selector(&self, event: &ClickEvent) -> String {
        // Check for desktop context first
        if let Some(metadata) = &event.metadata.ui_element {
            let app_name = metadata.application_name();
            let window_title = metadata.window_title();

            if self.is_desktop_context(&app_name, &window_title) {
                // Desktop-specific selector generation - prefer child text
                if !event.child_text_content.is_empty() {
                    return format!(
                        "desktop:{}|{}",
                        event.element_role, event.child_text_content[0]
                    );
                } else if !event.element_text.is_empty() {
                    return format!("desktop:{}|{}", event.element_role, event.element_text);
                } else {
                    return format!("desktop:role:{}", event.element_role);
                }
            }
        }

        // Regular application selector generation - trust the deepest element finder's result
        // Since our deepest element finder already performed coordinate checking,
        // we should use the actual clicked element's text, not child text from elements
        // that may not be under the click coordinates.

        if !event.element_text.is_empty() {
            // Use the actual clicked element's text
            format!("{}|{}", event.element_role, event.element_text)
        } else {
            // If the clicked element has no text, try child text as a fallback
            // (but only if we have child text and it's not from a large container)
            if !event.child_text_content.is_empty() && event.child_text_content.len() < 5 {
                let child_text = &event.child_text_content[0];
                // Only use child text if it's concise and specific
                if child_text.len() < 50 && !child_text.to_lowercase().contains("click") {
                    format!("{}|{}", event.element_role, child_text)
                } else {
                    // Child text is too verbose, use role-only selector
                    format!("role:{}", event.element_role)
                }
            } else {
                // No usable text, use role-only selector
                format!("role:{}", event.element_role)
            }
        }
    }

    /// Generate scoped selector using >> operator for better targeting
    fn generate_scoped_selector(
        &self,
        event: &ClickEvent,
        window_context: &Option<(String, String, String)>,
    ) -> String {
        // If we have window context, use scoped selector with >> operator
        if let Some((app_name, window_title, window_role)) = window_context {
            let window_selector =
                self.generate_window_selector(app_name, window_title, window_role);
            let element_selector = self.generate_element_selector(event);

            // Generate scoped selector: window >> element
            format!("{} >> {}", window_selector, element_selector)
        } else {
            // Fallback to basic selector if no window context
            self.generate_element_selector(event)
        }
    }

    /// Generate window selector for scoped search using actual detected role
    fn generate_window_selector(
        &self,
        app_name: &str,
        window_title: &str,
        window_role: &str,
    ) -> String {
        // Desktop-specific window selector
        if self.is_desktop_context(app_name, window_title) {
            return format!("role:{}|name:Desktop", window_role);
        }

        // CHROME-SPECIFIC FIX: Override detected role for Chrome applications
        // Chrome applications should use "Pane" selectors even if window() returns "Window"
        let role = if app_name.to_lowercase().contains("chrome") {
            tracing::info!(
                "🎯 Chrome detected - using role:Pane instead of role:{}",
                window_role
            );
            "Pane"
        } else if window_role.is_empty() {
            "Window"
        } else {
            window_role
        };

        // Extract meaningful title part from window title
        if let Some(title_part) = self.extract_meaningful_title(window_title) {
            format!("role:{}|name:contains:{}", role, title_part)
        } else {
            // App name-based window selector for regular applications
            match app_name.to_lowercase().as_str() {
                name if name.contains("chrome") => format!("role:{}|name:contains:Chrome", role),
                name if name.contains("firefox") => format!("role:{}|name:contains:Firefox", role),
                name if name.contains("edge") => format!("role:{}|name:contains:Edge", role),
                _ => format!("role:{}|name:contains:{}", role, app_name),
            }
        }
    }

    /// Generate element selector part for scoped search
    fn generate_element_selector(&self, event: &ClickEvent) -> String {
        // Use element text if available and meaningful
        if !event.element_text.is_empty() {
            format!("role:{}|name:{}", event.element_role, event.element_text)
        } else if !event.child_text_content.is_empty() && event.child_text_content.len() < 5 {
            // Use child text if concise and specific
            let child_text = &event.child_text_content[0];
            if child_text.len() < 50 && !child_text.to_lowercase().contains("click") {
                format!("role:{}|name:{}", event.element_role, child_text)
            } else {
                // Text is too verbose, try text selector
                format!("text:{}", child_text)
            }
        } else {
            // No usable text, use role-only selector
            format!("role:{}", event.element_role)
        }
    }

    /// Generate activation step for window/application targeting
    #[allow(dead_code)] // TODO: Will be used for application switching
    fn generate_activation_step(
        &self,
        app_name: &str,
        window_title: &str,
        window_role: &str,
    ) -> McpToolStep {
        let selector = self.generate_activation_selector(app_name, window_title, window_role);

        McpToolStep {
            tool_name: "activate_element".to_string(),
            arguments: json!({
                "selector": selector,
                "timeout_ms": 2000
            }),
            description: format!("Activate {} window", app_name),
            timeout_ms: Some(2000),
            continue_on_error: Some(false),
            delay_ms: Some(100),
        }
    }

    /// Generate activation selector based on app name and window title with actual role
    #[allow(dead_code)] // TODO: Will be used for application switching
    fn generate_activation_selector(
        &self,
        app_name: &str,
        window_title: &str,
        window_role: &str,
    ) -> String {
        // Desktop-specific activation
        if self.is_desktop_context(app_name, window_title) {
            return format!("role:{}|name:Desktop", window_role);
        }

        // Use the ACTUAL detected role instead of hardcoding "Window"
        let role = if window_role.is_empty() {
            "Window"
        } else {
            window_role
        };

        // Extract meaningful title part from window title
        if let Some(title_part) = self.extract_meaningful_title(window_title) {
            format!("role:{}|name:contains:{}", role, title_part)
        } else {
            // App name-based activation for regular applications
            match app_name.to_lowercase().as_str() {
                name if name.contains("chrome") => format!("role:{}|name:contains:Chrome", role),
                name if name.contains("firefox") => format!("role:{}|name:contains:Firefox", role),
                name if name.contains("edge") => format!("role:{}|name:contains:Edge", role),
                _ => format!("role:{}|name:{}", role, app_name),
            }
        }
    }

    /// Extract meaningful title part from full window title
    fn extract_meaningful_title(&self, full_title: &str) -> Option<String> {
        // Split on common patterns: " - ", " – ", " | "
        let separators = [" - ", " – ", " | "];

        for separator in &separators {
            if let Some(title_part) = full_title.split(separator).next() {
                let trimmed = title_part.trim();
                // Only use if it's meaningful (more than 3 chars and not generic)
                if trimmed.len() > 3
                    && !trimmed.to_lowercase().contains("new tab")
                    && !trimmed.to_lowercase().contains("untitled")
                {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }

    /// Detect if the click is happening in desktop context
    fn is_desktop_context(&self, app_name: &str, window_title: &str) -> bool {
        let app_lower = app_name.to_lowercase();
        let title_lower = window_title.to_lowercase();

        (app_lower.contains("explorer") && title_lower.contains("desktop")) ||
        app_lower.contains("dwm") ||           // Desktop Window Manager
        app_lower.contains("shell") ||         // Windows Shell  
        title_lower == "desktop" ||            // Direct desktop window
        app_lower.contains("progman") // Program Manager (desktop)
    }

    /// Generate fallback sequences for element clicks
    #[allow(dead_code)] // TODO: Will be used for robust click fallback strategies
    async fn generate_click_fallbacks(
        &self,
        event: &ClickEvent,
        _ui_context: Option<&EnhancedUIElement>,
    ) -> Result<Vec<Vec<McpToolStep>>> {
        let mut fallbacks = Vec::new();

        // Fallback 1: Name-only selector (parent element text)
        if !event.element_text.is_empty() {
            fallbacks.push(vec![McpToolStep {
                tool_name: "click_element".to_string(),
                arguments: json!({
                    "selector": format!("name:{}", event.element_text)
                }),
                description: format!("Click element by name: {}", event.element_text),
                timeout_ms: Some(5000),
                continue_on_error: Some(false),
                delay_ms: Some(200),
            }]);
        }

        // Fallback 2: Child text-based selectors (NEW ENHANCEMENT!)
        for (i, child_text) in event.child_text_content.iter().enumerate() {
            if !child_text.is_empty() {
                // Child text with role selector
                fallbacks.push(vec![McpToolStep {
                    tool_name: "click_element".to_string(),
                    arguments: json!({
                        "selector": format!("{}|{}", event.element_role, child_text)
                    }),
                    description: format!(
                        "Click {} containing '{}'",
                        event.element_role, child_text
                    ),
                    timeout_ms: Some(5000),
                    continue_on_error: Some(false),
                    delay_ms: Some(200),
                }]);

                // Child text with contains selector for broader matching
                fallbacks.push(vec![McpToolStep {
                    tool_name: "click_element".to_string(),
                    arguments: json!({
                        "selector": format!("contains:{}", child_text)
                    }),
                    description: format!("Click element containing text: {}", child_text),
                    timeout_ms: Some(5000),
                    continue_on_error: Some(false),
                    delay_ms: Some(200),
                }]);

                // Limit to first 2 child texts to avoid too many fallbacks
                if i >= 1 {
                    break;
                }
            }
        }

        // Removed overly broad role-only fallback for safety
        // (Previously: format!("role:{}", event.element_role) - too generic)

        Ok(fallbacks)
    }
}

/// Internal strategy result for text input conversion
struct TextInputStrategy {
    sequence: Vec<McpToolStep>,
    semantic_action: String,
    fallbacks: Vec<Vec<McpToolStep>>,
}

impl McpConverter {
    /// Convert suggestion-based text input (dropdown/autocomplete)
    async fn convert_suggestion_input(
        &self,
        event: &TextInputCompletedEvent,
        ui_context: Option<&EnhancedUIElement>,
    ) -> Result<TextInputStrategy> {
        let mut sequence = Vec::new();
        let mut notes = Vec::new();

        // Analyze UI pattern if context is available
        if let Some(context) = ui_context {
            if context.interaction_context.ui_pattern == "dropdown" {
                notes.push("Detected dropdown pattern".to_string());
                return self.generate_dropdown_sequence(event, context).await;
            } else if context.interaction_context.ui_pattern == "autocomplete" {
                notes.push("Detected autocomplete pattern".to_string());
                return self.generate_autocomplete_sequence(event, context).await;
            }
        }

        // Fallback: simple menu selection
        sequence.push(McpToolStep {
            tool_name: "click_element".to_string(),
            arguments: json!({
                "selector": format!("MenuItem|{}", event.text_value)
            }),
            description: format!("Select '{}' from menu", event.text_value),
            timeout_ms: Some(5000),
            continue_on_error: Some(false),
            delay_ms: Some(300),
        });

        Ok(TextInputStrategy {
            sequence,
            semantic_action: "menu_selection".to_string(),
            fallbacks: vec![],
        })
    }

    /// Convert typed text input
    async fn convert_typed_input(
        &self,
        event: &TextInputCompletedEvent,
        ui_context: Option<&EnhancedUIElement>,
    ) -> Result<TextInputStrategy> {
        let mut sequence = Vec::new();

        // Generate selector for the input field
        let selector = if let Some(context) = ui_context {
            context
                .suggested_selectors
                .first()
                .cloned()
                .unwrap_or_else(|| self.generate_text_field_selector(event))
        } else {
            self.generate_text_field_selector(event)
        };

        // Step 1: Focus the field
        sequence.push(McpToolStep {
            tool_name: "click_element".to_string(),
            arguments: json!({
                "selector": selector.clone()
            }),
            description: format!(
                "Focus {} field",
                event.field_name.as_deref().unwrap_or("text")
            ),
            timeout_ms: Some(3000),
            continue_on_error: Some(false),
            delay_ms: Some(100),
        });

        // Step 2: Type the text
        sequence.push(McpToolStep {
            tool_name: "type_into_element".to_string(),
            arguments: json!({
                "selector": selector,
                "text_to_type": event.text_value,
                "clear_before_typing": true
            }),
            description: format!("Type '{}' into field", event.text_value),
            timeout_ms: Some(5000),
            continue_on_error: Some(false),
            delay_ms: Some(200),
        });

        Ok(TextInputStrategy {
            sequence,
            semantic_action: "text_input".to_string(),
            fallbacks: vec![],
        })
    }

    /// Convert pasted text input
    async fn convert_pasted_input(
        &self,
        event: &TextInputCompletedEvent,
        ui_context: Option<&EnhancedUIElement>,
    ) -> Result<TextInputStrategy> {
        // Similar to typed input but potentially faster/different method
        self.convert_typed_input(event, ui_context).await
    }

    /// Convert fallback text input
    async fn convert_fallback_input(
        &self,
        event: &TextInputCompletedEvent,
        ui_context: Option<&EnhancedUIElement>,
    ) -> Result<TextInputStrategy> {
        // Use typed input as fallback
        self.convert_typed_input(event, ui_context).await
    }

    /// Generate dropdown interaction sequence
    async fn generate_dropdown_sequence(
        &self,
        event: &TextInputCompletedEvent,
        context: &EnhancedUIElement,
    ) -> Result<TextInputStrategy> {
        let mut sequence = Vec::new();

        // Step 1: Click dropdown trigger
        if let Some(trigger_selector) = self.find_dropdown_trigger(context) {
            sequence.push(McpToolStep {
                tool_name: "click_element".to_string(),
                arguments: json!({
                    "selector": trigger_selector
                }),
                description: "Open dropdown menu".to_string(),
                timeout_ms: Some(5000),
                continue_on_error: Some(false),
                delay_ms: Some(500),
            });
        }

        // Step 2: Select from dropdown
        let item_selector = format!("MenuItem|{}", event.text_value);
        sequence.push(McpToolStep {
            tool_name: "click_element".to_string(),
            arguments: json!({
                "selector": item_selector
            }),
            description: format!("Select '{}' from dropdown", event.text_value),
            timeout_ms: Some(5000),
            continue_on_error: Some(false),
            delay_ms: Some(200),
        });

        Ok(TextInputStrategy {
            sequence,
            semantic_action: "select_from_dropdown".to_string(),
            fallbacks: vec![],
        })
    }

    /// Generate autocomplete interaction sequence
    async fn generate_autocomplete_sequence(
        &self,
        event: &TextInputCompletedEvent,
        context: &EnhancedUIElement,
    ) -> Result<TextInputStrategy> {
        let mut sequence = Vec::new();

        // For autocomplete, we might need to type partial text then select
        let selector = context
            .suggested_selectors
            .first()
            .cloned()
            .unwrap_or_else(|| self.generate_text_field_selector(event));

        // Step 1: Focus field and type partial text to trigger autocomplete
        sequence.push(McpToolStep {
            tool_name: "click_element".to_string(),
            arguments: json!({
                "selector": selector.clone()
            }),
            description: "Focus autocomplete field".to_string(),
            timeout_ms: Some(3000),
            continue_on_error: Some(false),
            delay_ms: Some(100),
        });

        // Step 2: Type to trigger autocomplete
        let partial_text = if event.text_value.len() > 3 {
            &event.text_value[..3] // Type first 3 characters
        } else {
            &event.text_value
        };

        sequence.push(McpToolStep {
            tool_name: "type_into_element".to_string(),
            arguments: json!({
                "selector": selector,
                "text_to_type": partial_text,
                "clear_before_typing": true
            }),
            description: format!("Type '{}' to trigger autocomplete", partial_text),
            timeout_ms: Some(3000),
            continue_on_error: Some(false),
            delay_ms: Some(500),
        });

        // Step 3: Select from autocomplete suggestions
        sequence.push(McpToolStep {
            tool_name: "click_element".to_string(),
            arguments: json!({
                "selector": format!("ListItem|{}", event.text_value)
            }),
            description: format!("Select '{}' from autocomplete", event.text_value),
            timeout_ms: Some(5000),
            continue_on_error: Some(false),
            delay_ms: Some(200),
        });

        Ok(TextInputStrategy {
            sequence,
            semantic_action: "autocomplete_selection".to_string(),
            fallbacks: vec![],
        })
    }

    /// Find dropdown trigger element
    fn find_dropdown_trigger(&self, context: &EnhancedUIElement) -> Option<String> {
        // Look for related elements that might be dropdown triggers
        for related in &context.interaction_context.related_elements {
            if related.role == "TabItem"
                && related
                    .name
                    .as_ref()
                    .map_or(false, |name| name.contains("expand"))
            {
                return related.suggested_selectors.first().cloned();
            }
            if related.role == "Button"
                && related.name.as_ref().map_or(false, |name| {
                    name.contains("dropdown") || name.contains("▼")
                })
            {
                return related.suggested_selectors.first().cloned();
            }
        }
        None
    }

    /// Generate selector for text input field
    fn generate_text_field_selector(&self, event: &TextInputCompletedEvent) -> String {
        if let Some(field_name) = &event.field_name {
            if !field_name.is_empty() {
                return format!("{}|{}", event.field_type, field_name);
            }
        }
        format!("role:{}", event.field_type)
    }
}
