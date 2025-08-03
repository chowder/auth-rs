use clap::{Parser, Subcommand};
use client::Client;
use console::style;
use error::AuthError;

mod browser;
mod client;
mod env;
mod error;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CommandLineArgs {
    #[command(subcommand)]
    command: AppCommand,
}

#[derive(Subcommand, Debug)]
enum AppCommand {
    /// Start the authentication flow to authorize with your Jagex account
    Authorize,

    /// List all characters associated with the authorized Jagex account
    #[command(name = "ls")]
    ListCharacters,

    /// Execute a program with Jagex session credentials (e.g., RuneLite, OSRS client)
    Exec {
        /// Character ID to use for authentication
        #[arg(short, long, help = "Character ID from 'ls' command")]
        character_id: String,
        /// Name or path of the executable to run
        exec: String,
        /// Arguments to pass to the program
        #[arg(help = "Additional arguments for the program")]
        args: Vec<String>,
    },

    /// Clear all stored authentication tokens and sessions
    Logout,
}

#[tokio::main]
async fn main() -> miette::Result<()> {
    miette::set_panic_hook();
    env_logger::init();
    let cli = CommandLineArgs::parse();

    match cli.command {
        AppCommand::Authorize => browser::authorize(),
        AppCommand::ListCharacters => {
            let client = Client::new();
            let accounts = client.accounts().await?;
            for account in accounts {
                println!(
                    "  {} {} (ID: {})",
                    style("•").cyan(),
                    style(&account.display_name).green().bold(),
                    style(account.account_id.to_string()).bold()
                );
            }
            Ok(())
        }
        AppCommand::Exec {
            character_id,
            exec,
            args,
        } => {
            let client = Client::new();
            let session = client.session()?;
            let accounts = client.accounts().await?;

            if let Some(account) = accounts.iter().find(|a| a.account_id == character_id) {
                std::env::set_var("JX_SESSION_ID", session.session_id);
                std::env::set_var("JX_CHARACTER_ID", &account.account_id);
                std::env::set_var("JX_DISPLAY_NAME", &account.display_name);

                let mut args_with_program = args.clone();
                args_with_program.insert(0, exec.clone());
                let error = exec::execvp(&exec, args_with_program);
                Err(AuthError::ExecError {
                    program: exec.clone(),
                    details: format!("System error (errno: {error})"),
                })
            } else {
                let available_chars = accounts
                    .iter()
                    .map(|a| format!("  • {} (ID: {})", a.display_name, a.account_id))
                    .collect::<Vec<_>>()
                    .join("\n");

                Err(AuthError::CharacterNotFound {
                    character_id: character_id.clone(),
                    available_chars,
                })
            }
        }
        AppCommand::Logout => {
            let client = Client::new();
            client.logout()
        }
    }.map_err(|error| {
        error.into()
    })
}
