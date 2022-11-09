use anyhow::{anyhow, bail, Result};
use crossterm::{
    event::{self, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};
use std::io::prelude::*;
use std::io::BufReader;
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::Block,
    Terminal,
};
use duct::cmd;

fn main() -> Result<()> {
    // Check args
    if std::env::args().skip(1).count() < 2 {
        bail!("view <left> <right>");
    }
    
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and run it
    let result = update(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen,)?;
    terminal.show_cursor()?;

    if let Err(error) = result {
        println!("{:?}", error)
    }

    Ok(())
}

#[derive(Clone, Default)]
struct State {
    left: String,
    right: String,
}

enum Event {
    Exit,
    Draw,
}

fn update<B: Backend + Send>(terminal: &mut Terminal<B>) -> Result<()> {
    // State
    let (tx, rx) = mpsc::channel::<Event>();
    let state = Arc::new(Mutex::new(State::default()));

    // Left screen
    let left_tx = tx.clone();
    let left_state = state.clone();
    let left_thread = thread::spawn(move || {
        let Some(left) = std::env::args().skip(1).nth(0) else {
            return;
        };
        let split = left.split_whitespace().collect::<Vec<_>>();
        let [command, args @ ..] = split.as_slice() else {
            return;
        };
        let cmd = cmd(*command, args);
        let Ok(reader) = cmd.stderr_to_stdout().reader() else {
            return;
        };
        let mut lines = BufReader::new(reader).lines();
        while let Some(Ok(line)) = lines.next() {
            let Ok(mut lock) = left_state.lock() else {
                return;
            };
            lock.left.push_str(&line);
            let Ok(_) = left_tx.send(Event::Draw) else {
                return;
            };
        }
    });

    // Right screen
    let right_tx = tx.clone();
    let right_state = state.clone();
    let right_thread = thread::spawn(move || loop {
        let Some(right) = std::env::args().skip(1).nth(1) else {
            return;
        };
        let split = right.split_whitespace().collect::<Vec<_>>();
        let [command, args @ ..] = split.as_slice() else {
            return;
        };
        let cmd = cmd(*command, args);
        let Ok(reader) = cmd.stderr_to_stdout().reader() else {
            return;
        };
        let mut lines = BufReader::new(reader).lines();
        while let Some(Ok(line)) = lines.next() {
            let Ok(mut lock) = right_state.lock() else {
                return;
            };
            lock.right.push_str(&line);
            let Ok(_) = right_tx.send(Event::Draw) else {
                return;
            };
        }
    });

    // Input thread
    let input_tx = tx.clone();
    let input_thread = thread::spawn(move || loop {
        if let Ok(crossterm::event::Event::Key(key)) = event::read() {
            if let KeyCode::Char('q') = key.code {
                let Ok(_) = input_tx.send(Event::Exit) else {
                    return;
                };
            }
        }
    });

    // Main loop
    'outer: loop {
        match rx.recv() {
            Ok(Event::Draw) => {
                terminal.draw(|f| {
                    // Declare layout
                    let chunks = Layout::default()
                        .direction(Direction::Horizontal)
                        .constraints(
                            [Constraint::Percentage(50), Constraint::Percentage(50)].as_ref(),
                        )
                        .split(f.size());

                    // Get output
                    let Ok(lock) = state.lock() else {
                        return;
                    };

                    // Draw left screen
                    let block = Block::default().title(lock.left.clone());
                    f.render_widget(block, chunks[0]);

                    // Draw right screen
                    let block = Block::default().title(lock.right.clone());
                    f.render_widget(block, chunks[1]);
                })?;
            }
            Ok(Event::Exit) => return Ok(()),
            Err(_) => break 'outer,
        }
    }

    // Wait for threads
    left_thread
        .join()
        .map_err(|_| anyhow!("Joining left thread failed"))?;
    right_thread
        .join()
        .map_err(|_| anyhow!("Joining right thread failed"))?;
    input_thread
        .join()
        .map_err(|_| anyhow!("Joining input thread failed"))?;

    Ok(())
}
