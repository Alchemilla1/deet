use crate::debugger_command::DebuggerCommand;
use crate::inferior::{Inferior, Status};
use rustyline::error::ReadlineError;
use rustyline::Editor;

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<()>,
    inferior: Option<Inferior>,
}

impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        // TODO (milestone 3): initialize the DwarfData

        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<()>::new();
        // Attempt to load history from ~/.deet_history if it exists
        let _ = readline.load_history(&history_path);

        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            inferior: None,
        }
    }

    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {
                DebuggerCommand::Run(args) => {
                    // If type run when there exists inferior, kill the child process.
                    if let Some(inferior) = &mut self.inferior {
                        inferior.kill().expect("inferior.kill wasn't running");
                    }
                    if let Some(inferior) = Inferior::new(&self.target, &args) {
                        // Create the inferior
                        self.inferior = Some(inferior);
                        self.continue_exec();
                    } else {
                        println!("Error starting subprocess");
                    }
                }

                DebuggerCommand::Continue => {
                    if let Some(_) = &self.inferior {
                        self.continue_exec();
                    } else {
                        // continue when there is no inferior
                        println!("There is no inferior running");
                    }
                }

                DebuggerCommand::Quit => {
                    // if there exists inferior, kill the child process
                    if let Some(inferior) = &mut self.inferior {
                        inferior.kill().expect("inferior.kill wasn't running");
                    }
                    return;
                }
            }
        }
    }

    pub fn continue_exec(&mut self) {
        if let Some(inferior) = &self.inferior {
            match inferior.continue_exec() {
                Ok(status) => match status {
                    Status::Exited(exit_status_code) => {
                        self.inferior = None;
                        println!("Child exited (status {})", exit_status_code);
                    }
                    Status::Signaled(signal) => {
                        self.inferior = None;
                        println!("Child exited (signal {})", signal);
                    }
                    Status::Stopped(signal, _) => {
                        println!("Child stopped (signal {})", signal)
                    }
                },
                Err(err) => println!("Inferior can't be woken up and execute: {}", err),
            }
        } else {
            println!("inferior_continue_exec failed: there is no inferior");
        }
    }
    /// This function prompts the user to enter a command, and continues re-prompting until the user
    /// enters a valid command. It uses DebuggerCommand::from_tokens to do the command parsing.
    ///
    /// You don't need to read, understand, or modify this function.
    fn get_next_command(&mut self) -> DebuggerCommand {
        loop {
            // Print prompt and get next line of user input
            match self.readline.readline("(deet) ") {
                Err(ReadlineError::Interrupted) => {
                    // User pressed ctrl+c. We're going to ignore it
                    println!("Type \"quit\" to exit");
                }
                Err(ReadlineError::Eof) => {
                    // User pressed ctrl+d, which is the equivalent of "quit" for our purposes
                    return DebuggerCommand::Quit;
                }
                Err(err) => {
                    panic!("Unexpected I/O error: {:?}", err);
                }
                Ok(line) => {
                    if line.trim().len() == 0 {
                        continue;
                    }
                    self.readline.add_history_entry(line.as_str());
                    if let Err(err) = self.readline.save_history(&self.history_path) {
                        println!(
                            "Warning: failed to save history file at {}: {}",
                            self.history_path, err
                        );
                    }
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if let Some(cmd) = DebuggerCommand::from_tokens(&tokens) {
                        return cmd;
                    } else {
                        println!("Unrecognized command.");
                    }
                }
            }
        }
    }
}
