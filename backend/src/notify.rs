use std::process::Command;

/// Triggers post-install desktop notification guiding user to YNAB Developer settings
pub fn send_setup_notification() -> Result<(), std::io::Error> {
    let ynab_dev_url = "https://app.ynab.com/settings/developer";
    let headline = "YNAB Pulse Setup";
    let description = "Click here to generate your Personal Access Token in YNAB Account Settings > Developer.";

    // Prefer omarchy-notification-send with interactive --exec action
    let omarchy_res = Command::new("omarchy-notification-send")
        .arg("--exec")
        .arg(format!("xdg-open {}", ynab_dev_url))
        .arg("--app-name")
        .arg("YNAB Pulse")
        .arg(headline)
        .arg(description)
        .spawn();

    if omarchy_res.is_err() {
        // Fallback to standard notify-send
        let _ = Command::new("notify-send")
            .arg("-a")
            .arg("YNAB Pulse")
            .arg(headline)
            .arg(description)
            .spawn();
    }

    Ok(())
}
