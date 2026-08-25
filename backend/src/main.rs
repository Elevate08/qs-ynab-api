mod aggregator;
mod api;
mod auth;
mod cache;
mod crypto;
mod models;
mod notify;
mod storage;
#[cfg(test)]
mod test_env;

use api::ApiError;
use clap::{Parser, Subcommand};
use serde_json::json;

#[derive(Parser)]
#[command(name = "ynab-cli")]
#[command(about = "Fast, secure backend engine for the Omarchy YNAB Pulse plugin")]
#[command(version)]
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
    /// Securely save a new Personal Access Token, read from stdin
    ///
    /// The token is never accepted as an argument: process arguments are
    /// readable by any local process via /proc/<pid>/cmdline and are recorded
    /// in shell history. Pipe it instead:  ynab-cli auth set < token.txt
    Set {
        /// Rejected on purpose - see the note above
        #[arg(hide = true)]
        token: Option<String>,
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
                    // A token that is present but unreadable is a different
                    // state from no token at all: the fix is to re-enter it,
                    // not to wonder why the panel forgot the connection.
                    Err(auth::AuthError::Undecryptable(e)) => {
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
                if token.is_some() {
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "error": "Refusing to read a token from the command line, where any local process can read it from /proc. Pipe it to stdin instead: ynab-cli auth set < token.txt"
                        })
                    );
                    std::process::exit(2);
                }

                let trimmed = match auth::read_token_from_stdin() {
                    Ok(t) => t,
                    Err(e) => {
                        println!("{}", json!({ "ok": false, "error": format!("{}", e) }));
                        std::process::exit(1);
                    }
                };

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

                // Revoking access must also erase the financial data already
                // fetched with that token, not just the credential.
                let cache_purged = cache::purge_cache().is_ok();
                println!(
                    "{}",
                    json!({ "ok": true, "cleared": true, "cache_purged": cache_purged })
                );
            }
        },
        Commands::Fetch { budget_id, force } => {
            if let Some(ref requested) = budget_id {
                if !api::is_valid_budget_id(requested) {
                    println!(
                        "{}",
                        json!({
                            "ok": false,
                            "authenticated": false,
                            "error": "Malformed --budget-id (expected a UUID)"
                        })
                    );
                    std::process::exit(2);
                }
            }

            let token = match auth::get_token() {
                Ok(t) => t,
                Err(e) => {
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
                            "error": format!("{}", e)
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

            // No /user call: its only consumer was the user_id that used to go
            // into the payload, so fetching it now would be a round trip to
            // collect an identifier nothing reads.
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

            // The four remaining calls depend only on the budget id, not on
            // each other, so they run together instead of one after another.
            //
            // They share the pooled client, which is `Sync`, so each thread
            // borrows it; `thread::scope` guarantees they finish before the
            // borrow ends, so nothing needs to be cloned or reference-counted.
            // Sequentially this was four round trips on a widget that opens on
            // a keypress.
            let budget_ref = &active_budget.id;
            let client_ref = &client;
            let (month_result, categories_result, months_result, unapproved_result) =
                std::thread::scope(|scope| {
                    let month = scope.spawn(move || client_ref.get_current_month(budget_ref));
                    let categories =
                        scope.spawn(move || client_ref.get_categories(budget_ref, None));
                    let months = scope.spawn(move || client_ref.get_months(budget_ref));
                    let unapproved =
                        scope.spawn(move || client_ref.get_unapproved_transactions(budget_ref));
                    (
                        month.join(),
                        categories.join(),
                        months.join(),
                        unapproved.join(),
                    )
                });

            // A panicking worker is reported like any other failure rather
            // than taking the process down with it.
            let panicked = |what: &str| {
                ApiError::NetworkError(format!("The {} request thread panicked", what))
            };

            // Checked in the same order as before, so a run that fails more
            // than one way reports the same error it always did.
            let month_resp = match month_result.unwrap_or_else(|_| Err(panicked("current month"))) {
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

            let categories_resp =
                match categories_result.unwrap_or_else(|_| Err(panicked("categories"))) {
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

            // Both of these are decoration: a failure costs the trend graph or
            // the review badge, not the panel.
            let months_history = months_result
                .ok()
                .and_then(|r| r.ok())
                .map(|w| w.months)
                .unwrap_or_default();

            let unapproved_count = unapproved_result
                .ok()
                .and_then(|r| r.ok())
                .map(|t| t.transactions.len())
                .unwrap_or(0);

            // Aggregate metrics into unified overview
            let overview = aggregator::build_overview_payload(
                &budgets_resp.budgets,
                active_budget,
                &month_resp.month,
                &months_history,
                &categories_resp.category_groups,
                unapproved_count,
                categories_resp.server_knowledge,
            );

            // Non-fatal - the fetch succeeded and the panel has its data - but
            // not silent, or a cache that stops updating looks like nothing at
            // all. stderr, because stdout is the JSON the panel parses.
            if let Err(e) = cache::write_cache(&overview) {
                eprintln!("ynab-cli: could not write the offline cache: {}", e);
            }

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
