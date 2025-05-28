use eframe::{
    egui, // Re-export egui from eframe for convenience
    App, NativeOptions,
};
use std::time::{Duration, Instant};
use rand::Rng;

fn main() -> Result<(), eframe::Error> {
    // Configure eframe native options
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_position([900.0, 800.0])
            .with_inner_size([300.0, 120.0]) // Adjusted window size
            .with_resizable(true)
            .with_decorations(false) // Hide window decorations
            .with_always_on_top(), // Keep window on top

        // By default, eframe tries `wgpu`, then `glow`, then `softbuffer`.
        // You could force software rendering if needed:
        // renderer: eframe::Renderer::Software,

        ..Default::default()
    };

    // Run the application
    eframe::run_native(
        "Button Example", // Window title (still used internally)
        options,
        Box::new(|_cc| Box::<MyApp>::default()), // App creation closure
    )
}

// Define the application state struct
struct MyApp {
    is_processing: bool,
    button_text: String,
    last_update_time: Option<Instant>,
    rng: rand::rngs::ThreadRng,
    processing_texts: Vec<String>,
    prompt_text: String, // Added field for prompt input
}

impl Default for MyApp {
    fn default() -> Self {
        Self {
            is_processing: false,
            button_text: "start processing".to_owned(),
            last_update_time: None,
            rng: rand::thread_rng(),
            processing_texts: vec![
                "analysing...".to_owned(),
                "processing...".to_owned(),
                "thinking...".to_owned(),
            ],
            prompt_text: String::new(), // Initialize prompt_text
        }
    }
}

// Implement the eframe App trait for our struct
impl App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame::none()) // Remove panel frame/padding
            .show(ctx, |ui| {
                let desired_size = ui.available_size();

                if self.is_processing {
                    // When processing, show a spinner in a yellow box instead of the button
                    let (rect, _) = ui.allocate_exact_size(desired_size, egui::Sense::hover());

                    // Draw background (yellow, like the button)
                    let visuals = ui.visuals().widgets.inactive; // Use inactive widget visuals for rounding
                    ui.painter().rect_filled(rect, visuals.rounding, egui::Color32::YELLOW);

                    // Calculate center for spinner and text
                    let center = rect.center();

                    // Add spinner (slightly above center to make space for text)
                    let spinner_pos = center - egui::vec2(0.0, 10.0); // Adjust vertical offset as needed
                    ui.put(egui::Rect::from_center_size(spinner_pos, egui::Vec2::splat(20.0)), egui::Spinner::new()); // Specify size for spinner

                    // Draw the processing text below the spinner
                    let text_pos = center + egui::vec2(0.0, 10.0); // Adjust vertical offset as needed
                    ui.painter().text(
                        text_pos,
                        egui::Align2::CENTER_CENTER,
                        &self.button_text, // Draw the current processing text
                        egui::FontId::proportional(16.0), // Choose font size
                        ui.visuals().text_color(), // Use default text color
                    );

                } else {
                    // When not processing, show input field and button
                    ui.vertical_centered_justified(|ui| { // Center and justify content
                        // Input field for the prompt
                        ui.add(
                            egui::TextEdit::multiline(&mut self.prompt_text)
                                .font(egui::FontId::new(16.0, egui::FontFamily::Monospace)) // Corrected font setting
                                .min_size(egui::vec2(ui.available_width(), 60.0)) // Set a minimum height
                                .desired_width(f32::INFINITY) // Fill available width
                                .hint_text("Enter your prompt here...")
                        );

                        ui.add_space(4.0); // Space between input and button

                        // Button
                        let button_text_display = egui::RichText::new(format!("\u{25B6} {}", self.button_text))
                            .size(20.0); // Font size for button text
                        
                        let start_button = egui::Button::new(button_text_display)
                            .fill(egui::Color32::YELLOW)
                            .min_size(egui::vec2(ui.available_width(), 30.0)); // Fill width, specific height

                        if ui.add(start_button).clicked() {
                            println!("Using prompt: '{}'", self.prompt_text); // Print the prompt
                            // Original button click logic
                            println!("Button clicked! Starting processing...");
                            self.is_processing = true;
                            self.last_update_time = Some(Instant::now());
                            let next_text_index = self.rng.gen_range(0..self.processing_texts.len());
                            self.button_text = self.processing_texts[next_text_index].clone();
                            // self.prompt_text.clear(); // Optional: clear prompt after submission
                        }
                    });
                }

                // Update text periodically if processing (logic moved outside the draw if/else)
                if self.is_processing {
                    let update_interval = Duration::from_secs(2); // Change text every 2 seconds

                    if let Some(last_update) = self.last_update_time {
                        if last_update.elapsed() >= update_interval {
                            let next_text_index = self.rng.gen_range(0..self.processing_texts.len());
                            self.button_text = self.processing_texts[next_text_index].clone(); // This text is not displayed visually but tracked
                            self.last_update_time = Some(Instant::now()); // Reset timer
                        }
                    }
                    // Request redraw to check time again soon
                    ctx.request_repaint_after(update_interval / 4);

                    // Add logic here to check if processing is finished
                    // If finished:
                    // self.is_processing = false;
                    // self.button_text = "start processing".to_owned();
                    // self.last_update_time = None;
                }
            });
    }
}
