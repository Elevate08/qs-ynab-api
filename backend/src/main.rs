mod aggregator;
mod api;
mod auth;
mod cache;
mod models;
mod notify;

use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "ynab-cli")]
#[command(about = "Fast, secure backend engine for the Omarchy YNAB Pulse plugin")]
#[command(version = "1.0.0")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Authentication management with Linux Secret Service keyring
    Auth {
        #[command(subcommand)]
        sub: AuthCommands,
    },
    /// Fetch and aggregate YNAB budget metrics, buckets, and pie chart data
    Fetch {
        /// Specific budget ID to fetch (defaults to default or most recent budget)
        #[arg(short, long)]
        budget_id: Option<String>,

        /// Force fresh network fetch, bypassing cache
        #[arg(short, long)]
        force: bool,
    },
    /// Trigger post-installation desktop notification
    NotifySetup,
}

#[derive(Subcommand)]
enum AuthCommands {
    /// Check if a valid token is present in the Secret Service keyring
    Status,
    /// Securely save a new Personal Access Token to the keyring
    Set {
        /// Personal access token from YNAB Developer settings
        token: String,
    },
    /// Remove the stored token from the keyring
    Clear,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Auth { sub } => match sub {
            AuthCommands::Status => {
                match auth::get_token() {
                    Ok(token) => {
                        // Validate token with live YNAB API call
                        match api::YnabClient::new(token) {
                            Ok(client) => match client.get_user() {
                                Ok(user) => {
                                    println!(
                                        "{}",
                                        json!({
                                            "ok": true,
                                            "authenticated": true,
                                            "has_token": true,
                                            "user_id": user.user.id
                                        })
                                    );
                                }
                                Err(e) => {
                                    println!(
                                        "{}",
                                        json!({
                                            "ok": true,
                                            "authenticated": false,
                                            "has_token": true,
                                            "error": format!("{}", e)
                                        })
                                    );
                                }
                            },
                            Err(e) => {
                                println!(
                                    "{}",
                                    json!({
                                        "ok": false,
                                        "authenticated": false,
                                        "has_token": true,
                                        "error": format!("{}", e)
                                    })
                                );
                            }
                        }
                    }
                    Err(_) => {
                        println!(
                            "{}",
                            json!({
                                "ok": true,
                                "authenticated": false,
                                "has_token": false
                            })
                        );
                    }
                }
            }
            AuthCommands::Set { token } => {
                let trimmed = token.trim().to_string();
                if trimmed.is_empty() || trimmed.len() < 10 {
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "error": "Invalid token length or format"
                        })
                    );
                    std::process::exit(1);
                }

                // Verify token with YNAB API before saving to keyring
                match api::YnabClient::new(trimmed.clone()) {
                    Ok(client) => match client.get_user() {
                        Ok(user) => {
                            if let Err(e) = auth::set_token(&trimmed) {
                                println!(
                                    "{}",
                                    json!({
                                        "ok": false,
                                        "error": format!("Failed to save token to keyring: {}", e)
                                    })
                                );
                                std::process::exit(1);
                            }

                            println!(
                                "{}",
                                json!({
                                    "ok": true,
                                    "saved": true,
                                    "user_id": user.user.id
                                })
                            );
                        }
                        Err(e) => {
                            println!(
                                "{}",
                                json!({
                                    "ok": false,
                                    "error": format!("Token verification failed: {}", e)
                                })
                            );
                            std::process::exit(1);
                        }
                    },
                    Err(e) => {
                        println!(
                            "{}",
                            json!({
                                "ok": false,
                                "error": format!("Client initialization error: {}", e)
                            })
                        );
                        std::process::exit(1);
                    }
                }
            }
            AuthCommands::Clear => {
                if let Err(e) = auth::clear_token() {
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "error": format!("Failed to clear token: {}", e)
                        })
                    );
                    std::process::exit(1);
                }
                println!("{}", json!({ "ok": true, "cleared": true }));
            }
        },
        Commands::Fetch { budget_id, force } => {
            let token = match auth::get_token() {
                Ok(t) => t,
                Err(_) => {
                    // Check if we have cached data to display offline
                    if !force {
                        if let Some(cached) = cache::read_cache() {
                            println!("{}", serde_json::to_string(&cached).unwrap());
                            return;
                        }
                    }
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "authenticated": false,
                            "error": "No YNAB Personal Access Token configured in Keyring"
                        })
                    );
                    return;
                }
            };

            let client = match api::YnabClient::new(token) {
                Ok(c) => c,
                Err(e) => {
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "authenticated": false,
                            "error": format!("Client initialization failed: {}", e)
                        })
                    );
                    return;
                }
            };

            // Fetch user info & budget list
            let user_info = client.get_user().ok();
            let budgets_resp = match client.get_budgets() {
                Ok(b) => b,
                Err(e) => {
                    if !force {
                        if let Some(cached) = cache::read_cache() {
                            println!("{}", serde_json::to_string(&cached).unwrap());
                            return;
                        }
                    }
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "authenticated": true,
                            "error": format!("Failed to fetch budgets: {}", e)
                        })
                    );
                    return;
                }
            };

            if budgets_resp.budgets.is_empty() {
                println!(
                    "{}",
                    json!({
                        "ok": false,
                        "authenticated": true,
                        "error": "No budgets found in this YNAB account"
                    })
                );
                return;
            }

            // Determine active budget
            let active_budget = if let Some(ref req_id) = budget_id {
                budgets_resp
                    .budgets
                    .iter()
                    .find(|b| &b.id == req_id)
                    .unwrap_or(&budgets_resp.budgets[0])
            } else if let Some(ref def_b) = budgets_resp.default_budget {
                def_b
            } else {
                &budgets_resp.budgets[0]
            };

            // Fetch current month summary
            let month_resp = match client.get_current_month(&active_budget.id) {
                Ok(m) => m,
                Err(e) => {
                    if !force {
                        if let Some(cached) = cache::read_cache() {
                            println!("{}", serde_json::to_string(&cached).unwrap());
                            return;
                        }
                    }
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "authenticated": true,
                            "error": format!("Failed to fetch current month: {}", e)
                        })
                    );
                    return;
                }
            };

            // Fetch categories for active budget
            let categories_resp = match client.get_categories(&active_budget.id, None) {
                Ok(c) => c,
                Err(e) => {
                    if !force {
                        if let Some(cached) = cache::read_cache() {
                            println!("{}", serde_json::to_string(&cached).unwrap());
                            return;
                        }
                    }
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "authenticated": true,
                            "error": format!("Failed to fetch categories: {}", e)
                        })
                    );
                    return;
                }
            };

            // Fetch multi-month history for trend graph
            let months_history = client
                .get_months(&active_budget.id)
                .map(|w| w.months)
                .unwrap_or_default();

            // Fetch unapproved transactions for review count
            let unapproved_count = client
                .get_unapproved_transactions(&active_budget.id)
                .ok()
                .map(|t| t.transactions.len())
                .unwrap_or(0);

            // Aggregate metrics into unified overview
            let overview = aggregator::build_overview_payload(
                user_info.map(|u| u.user.id),
                &budgets_resp.budgets,
                active_budget,
                &month_resp.month,
                &months_history,
                &categories_resp.category_groups,
                unapproved_count,
                categories_resp.server_knowledge,
            );

            // Persist to secure cache
            let _ = cache::write_cache(&overview);

            println!("{}", serde_json::to_string_pretty(&overview).unwrap());
        }
        Commands::NotifySetup => {
            if let Err(e) = notify::send_setup_notification() {
                eprintln!("Failed to send desktop notification: {}", e);
                std::process::exit(1);
            }
            println!("{}", json!({ "ok": true, "notified": true }));
        }
    }
}
