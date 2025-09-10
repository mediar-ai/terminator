fn main() {
    tracing_subscriber::fmt::init();
    match virtual_display_win::enumerate_displays() {
        Ok(displays) => {
            println!("Found {} active display path(s)", displays.len());
            for (idx, d) in displays.iter().enumerate() {
                println!(
                    "#{}: adapter={:08x}:{:08x} target_id={} active={} {}x{} @ {} Hz name={}",
                    idx,
                    d.adapter_id_high,
                    d.adapter_id_low,
                    d.target_id,
                    d.is_active,
                    d.width,
                    d.height,
                    d.refresh_hz,
                    d.friendly_name.as_deref().unwrap_or("<unknown>")
                );
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}
