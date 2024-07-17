use crate::debugger_command::DebuggerCommand;
use crate::dwarf_data::{DwarfData, Error as DwarfError};
use crate::inferior::{Inferior, Status};
// use libc::getaddrinfo;
// use nix::sys::ptrace;
use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::Editor;
use std::mem::size_of;

#[derive(Clone)]
pub struct Breakpoint {
    pub addr: usize,
    pub orig_byte: u8,
}

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<(), FileHistory>,
    inferior: Option<Inferior>,
    debug_data: DwarfData,
    breakpoints: Vec<usize>,
}

fn parse_address(addr: &str) -> Option<usize> {
    // TODO(milestore 6): update this code to take different kinds of breakpoints
    // ensure the addr starts with "*"
    let addr = if addr.to_lowercase().starts_with("*") {
        &addr[1..]
    } else {
        &addr
    };
    let addr_without_0x = if addr.to_lowercase().starts_with("0x") {
        &addr[2..]
    } else {
        &addr
    };
    // println!("addr = {}", addr);
    usize::from_str_radix(addr_without_0x, 16).ok()
}


impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        let debug_data = match DwarfData::from_file(target) {
            Ok(val) => val,
            Err(DwarfError::ErrorOpeningFile) => {
                println!("Could not open file {}", target);
                std::process::exit(1);
            }
            Err(DwarfError::DwarfFormatError(err)) => {
                println!("Could not debugging symbols from {}: {:?}", target, err);
                std::process::exit(1);
            }
        };
        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<(), FileHistory>::new().expect("Create Editor fail");
        // Attempt to load history from ~/.deet_history if it exists
        let _ = readline.load_history(&history_path);
        debug_data.print();

        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            inferior: None,
            debug_data,
            breakpoints: vec![],
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
                    if let Some(inferior) = Inferior::new(&self.target, &args, &self.breakpoints) {
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

                DebuggerCommand::Backtrace => {
                    if let Some(inferior) = &self.inferior {
                        inferior.print_backtrace(&self.debug_data).unwrap();
                    }
                }

                DebuggerCommand::Quit => {
                    // if there exists inferior, kill the child process
                    if let Some(inferior) = &mut self.inferior {
                        inferior.kill().expect("inferior.kill wasn't running");
                    }
                    return;
                }

                DebuggerCommand::Breakpoint(breakpoint) => {
                    match parse_address(&breakpoint) {
                        Some(addr_usize) => { 
                            println!("Set breakpoint {} at {}", self.breakpoints.len(), addr_usize);
                            self.breakpoints.push(addr_usize);
                        }
                        None => println!("fail to parse a usize from a hexadecimal string"),
                    }
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
                    Status::Stopped(signal, rip) => {
                        println!("Child stopped (signal {})", signal);
                        if let Some(line) = self.debug_data.get_line_from_addr(rip) {
                            println!("Stopped at {}", line);
                        }
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
