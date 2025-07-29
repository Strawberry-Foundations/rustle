use libstrawberry::notifications::Notifier;

pub fn show_transfer_notification(sender: &str, filename: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let notifier = Notifier::new(
        "File Transfer Request",
        format!("{sender} wants to send you '{filename}'"),
        "Rustle",
        "normal",
        "/usr/share/icons/hicolor/48x48/apps/folder.png",
        None,
        30000,
        false,
    ).build();

    let actions = vec![
        ("accept".to_string(), "Accept".to_string()),
        ("decline".to_string(), "Decline".to_string()),
    ];

    match notifier.send_with_actions_and_wait(actions)? {
        Some(action) => Ok(action == "accept"),
        None => Ok(false), 
    }
}