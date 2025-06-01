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

    // let mut collected_texts = Vec::new();
    let chat_list = Selector::Role {
        role: "list".to_string(),
        name: Some("Chats list".to_string()),
    };

    let chat_list_ele = engine.find_element(&chat_list, Some(&wh_root), None)?;

    let chat_list_person = Selector::Role { 
        role: "listitem".to_string(), 
        name: Some("raushan".to_string()),        // replace with the person's name, you wanna scrap the chat
    };

    let chat_list_person_ele = engine.find_element(&chat_list_person, Some(&chat_list_ele), None)?;

    chat_list_person_ele.click()?; // open the person's chat

    std::thread::sleep(std::time::Duration::from_millis(200));

    let msg_list = Selector::Role {  // that person's messages
        role: "list".to_string(),
        name: Some("Messages".to_string()),
    };

    let messages_list = engine.find_element(&msg_list, Some(&wh_root), None)?;

    let individual_msg_block = Selector::UINativeId("BubbleListItem".to_string());

    let individual_msg_block_eles = engine.find_elements(&individual_msg_block, Some(&messages_list), None, Some(usize::MAX))?;
    
    for individual_msg_block_ele in individual_msg_block_eles {
        let bubble_list_name = individual_msg_block_ele.name()
            .expect("failed to get `individual_msg_block_ele` name");

        if bubble_list_name.contains(":") {
            let parts: Vec<&str> = bubble_list_name.splitn(2, ':').collect();             // part 1 is replier
            let replier = parts[1].to_string();

            let chat_text_classname = Selector::ClassName("RichTextBlock".to_string());
            // let chat_text_ele = engine.find_element(&chat_text_classname, Some(&individual_msg_block_ele), Some(std::time::Duration::from_millis(2000)))?; // messages_list as root
            
            if let Ok(chat_text_ele) = engine.find_element(&chat_text_classname, Some(&individual_msg_block_ele), Some(std::time::Duration::from_millis(2000))) {
                if chat_text_ele.attributes().ui_native_id == Some("TextBlock".to_string()) {
                    println!("REPLIER: {:?}, MESSAGE TEXT: {:?}", replier, chat_text_ele.name());  
                    // if let Some(text) = chat_text_ele.name() {
                    //     collected_texts.push(text.clone());
                    // }

                    let json_data = json!({ "messages": chat_text_ele.name() });
                    let mut file = File::create("messages.json").expect("failed json");
                    file.write_all(json_data.to_string().as_bytes()).expect("failed to write json");

                    // for text in &collected_texts {
                    //     println!("{}", text);
                    // }
                }
            } else {
                println!("skipping bubbleListItem as it does not contain a RichTextBlock, maybe its gif");
            }
        } else {
            println!("skipping might be date");
        }
    }


    // for text in &collected_texts {
    //     println!("{}", text);
    // }

    // let mut collected_texts = Vec::new();
    // for actual_chat_text in chat_text_eles {
    //     // aviod duplicate texts
    //     if actual_chat_text.attributes().ui_native_id == Some("TextBlock".to_string()) {
    //         println!("MESSAGE TEXT: {:?}", actual_chat_text.name());            // the name of the first text from list item would be the message
    //         if let Some(text) = actual_chat_text.name() {
    //             collected_texts.push(text.clone());
    //         }
    //     }
    // }




    // let msg_list_items = Selector::Role { 
    //     role: "listitem".to_string(), 
    //     name: None,
    // };
    //
    // let messages_list_items = engine.find_elements(&msg_list_items, Some(&messages_list), None, Some(usize::MAX))?; // messages_list as root
    // println!("message list {:?}", messages_list_items);
    //
    // let msg_text = Selector::Role { 
    //     role: "text".to_string(), 
    //     name: None,
    // };
    //
    // let mut collected_texts = Vec::new();
    // for messages_list_item in messages_list_items {
    //     let messages_text = engine.find_element(&msg_text, Some(&messages_list_item), None)?; // messages_list_items as root
    //     println!("MESSAGE TEXT: {:?}", messages_text.name());            // the name of the first text from list item would be the message
    //     if let Some(text) = messages_text.name() {
    //         collected_texts.push(text.clone());
    //     }
    // }
    //

    Ok(())
}

