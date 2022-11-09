use anyhow::{Result, bail, anyhow};
use crossterm::{
    event::{self, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::Block,
    Frame, Terminal,
};
use std::{sync::{mpsc, Arc, Mutex}, thread, time::Duration};

fn main() -> Result<()> {
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
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
    )?;
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
    let commands = std::env::args().skip(1).take(2).collect::<Vec<_>>();
    let [left, right] = commands.as_slice() else {
        bail!("view <left> <right>");
    };
    let state = Arc::new(Mutex::new(State::default()));
    let (tx, rx) = mpsc::channel::<Event>();

    // Left screen
    let left_tx = tx.clone();
    let left_state = state.clone();
    let left_thread = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(1000));
            let Ok(mut lock) = left_state.lock() else {
                continue;
            };
            lock.left.push_str("left\n");
            left_tx.send(Event::Draw).unwrap();
        }
    });

    // Right screen
    let right_tx = tx.clone();
    let right_state = state.clone();
    let right_thread = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(1000));
            let Ok(mut lock) = right_state.lock() else {
                continue;
            };
            lock.right.push_str("right\n");
            right_tx.send(Event::Draw).unwrap();
        }
    });

    // Input thread
    let input_tx = tx.clone();
    let input_thread = thread::spawn(move || {
        loop {
            if let Ok(crossterm::event::Event::Key(key)) = event::read() {
                if let KeyCode::Char('q') = key.code {
                    input_tx.send(Event::Exit).unwrap();
                }
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
                        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
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
            },
            Ok(Event::Exit) => return Ok(()),
            Err(_) => break 'outer,
        }
    }

    // Wait for threads
    left_thread.join().map_err(|_| anyhow!("Joining left thread failed"))?;
    right_thread.join().map_err(|_| anyhow!("Joining right thread failed"))?;
    input_thread.join().map_err(|_| anyhow!("Joining input thread failed"))?;

    Ok(())
}
