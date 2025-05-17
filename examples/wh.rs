// simple example to scrap whatsapp
use std::{hash::Hash, thread};
use std::time::Duration;

use terminator::{platforms, AutomationError, Desktop, Selector};
use tracing::Level;

#[tokio::main]
async fn main() -> Result<(), AutomationError> {

    let engine = platforms::create_engine(true, true)?;
    tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(Level::DEBUG)
        .init();

    let wh_root = engine.open_application("whatsapp")?;
    println!("opened application: {:?}", wh_root);

    std::thread::sleep(std::time::Duration::from_millis(500));

    // when the element is name is blank we can get its parent element id then we can retrive the children id
    // we can also create a Selector with the role (control_type) with name on it
    let ch_list = Selector::Role {
        role: "list".to_string(),
        name: Some("Chats list".to_string()),
    };

    let grp_per = Selector::Role { 
        role: "listitem".to_string(), 
        name: Some("tribhuwan".to_string()),
    };

    let ch_list_ele = engine.find_element(&ch_list, Some(&wh_root), None)?;
    println!("CHAT ELE: {:#?}", ch_list_ele);
    let grp_per_ele = engine.find_element(&grp_per, Some(&ch_list_ele), None)?;

    grp_per_ele.click()?;
    std::thread::sleep(std::time::Duration::from_millis(200));

    let msg_list = Selector::Role {
        role: "list".to_string(),
        name: Some("Messages".to_string()),
    };

    let messages_list = engine.find_element(&msg_list, Some(&wh_root), None)?;

    // we might need a algo for determining the text depth
    let child_mes = messages_list.text(100);
    println!("TEXT: {:#?}", child_mes);

    Ok(())
}
