fn main() {
    tracing_subscriber::fmt::init();

    let has_driver = match virtual_display_win::is_virtual_driver_present() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("driver check error: {e}");
            std::process::exit(3);
        }
    };

    let displays = match virtual_display_win::enumerate_displays() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("enumeration error: {e}");
            std::process::exit(4);
        }
    };

    let active_count = displays.iter().filter(|d| d.is_active).count();

    println!(
        "virtual_driver_present={} active_displays={}",
        has_driver, active_count
    );

    if !has_driver {
        std::process::exit(1);
    }
    if active_count == 0 {
        std::process::exit(2);
    }
}

