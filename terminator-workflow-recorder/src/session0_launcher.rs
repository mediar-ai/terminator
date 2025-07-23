use std::process::Command;

pub fn launch_in_session0(app: &str) {
    println!("Launching '{}' in Session 0...", app);
    let _ = Command::new(app).spawn();
}
