// example for scrapping whatsapp
use tracing::Level;
use std::fs::File;
use std::io::Write;
use serde_json::json;
use terminator::{platforms, AutomationError, Selector};

#[tokio::main]
async fn main() -> Result<(), AutomationError> {

    let engine = platforms::create_engine(true, true)?;
    tracing_subscriber::fmt::Subscriber::builder()
        .with_max_level(Level::DEBUG)
        .init();

    let wh_root = engine.open_application("uwp:whatsapp")?;
    println!("opened application: {:?}", wh_root);

    std::thread::sleep(std::time::Duration::from_millis(500));

    let ch_list = Selector::Role {
        role: "list".to_string(),
        name: Some("Chats list".to_string()),
    };

    let grp_person = Selector::Role { 
        role: "listitem".to_string(), 
        name: Some("hero".to_string()),        // replace with the person's name, you wanna scrap the chat
    };


    let ch_list_ele = engine.find_element(&ch_list, Some(&wh_root), None)?;
    println!("CHAT ELE: {:?}", ch_list_ele);
    let grp_per_ele = engine.find_element(&grp_person, Some(&ch_list_ele), None)?;

    grp_per_ele.click()?;
    std::thread::sleep(std::time::Duration::from_millis(200));

    let msg_list = Selector::Role {
        role: "list".to_string(),
        name: Some("Messages".to_string()),
    };

    let messages_list = engine.find_element(&msg_list, Some(&wh_root), None)?;
    println!("Messages list, {:?}", messages_list);

    let msg_list_items = Selector::Role { 
        role: "listitem".to_string(), 
        name: None,
    };

    let messages_list_items = engine.find_elements(&msg_list_items, Some(&messages_list), None, Some(usize::MAX))?; // messages_list as root
    println!("message list {:?}", messages_list_items);

    let msg_text = Selector::Role { 
        role: "text".to_string(), 
        name: None,
    };

    let mut collected_texts = Vec::new();
    for messages_list_item in messages_list_items {
        let messages_text = engine.find_element(&msg_text, Some(&messages_list_item), None)?; // messages_list_items as root
        println!("MESSAGE TEXT: {:?}", messages_text.name());            // the name of the first text from list item would be the message
        if let Some(text) = messages_text.name() {
            collected_texts.push(text.clone());
        }
    }

    let json_data = json!({ "messages": collected_texts });
    let mut file = File::create("messages.json").expect("failed json");
    file.write_all(json_data.to_string().as_bytes()).expect("failed to write json");

    for text in &collected_texts {
        println!("{}", text);
    }

    Ok(())
}

