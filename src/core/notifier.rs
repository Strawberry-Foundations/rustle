use libstrawberry::notifications::Notifier;
use crate::core::I18N;

pub fn show_transfer_notification(sender: &str, filename: &str) -> Result<bool, Box<dyn std::error::Error>> {
    let title = I18N.get("notifier_title");
    let body = I18N.get_with_params("notifier_body", &[&sender, &filename]);
    let accept = I18N.get("notifier_accept");
    let decline = I18N.get("notifier_decline");

    let notifier = Notifier::new(
        &title,
        body,
        "Rustle",
        "critical",
        "/home/julian/Projects/rustle/rustle.png",
        None,
        0,
        false,
    ).build();

    let actions = vec![
        ("accept".to_string(), accept),
        ("decline".to_string(), decline),
    ];

    match notifier.send_with_actions_and_wait(actions)? {
        Some(action) => Ok(action == "accept"),
        None => Ok(false),
    }
}

pub fn show_simple(title: &str, body: &str, is_error: bool) {
    let urgency = if is_error { "critical" } else { "normal" };
    let icon = "/home/julian/Projects/rustle/rustle.png";

    let body_owned = body.to_string();
    let title_owned = title.to_string();
    let urgency_owned = urgency.to_string();

    std::thread::spawn(move || {
        let notifier = Notifier::new(
            &title_owned,
            body_owned,
            "Rustle",
            &urgency_owned,
            icon,
            None,
            0,
            false,
        ).build();
        
        let _ = notifier.send_with_actions_and_wait(vec![]);
    });
}