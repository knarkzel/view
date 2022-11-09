use anyhow::{Result, bail};
use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use tui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    widgets::Block,
    Frame, Terminal,
};
use std::{thread, sync::{Arc, Mutex}, time::Duration};

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

fn update<B: Backend>(terminal: &mut Terminal<B>) -> Result<()> {
    // State
    let commands = std::env::args().skip(1).take(2).collect::<Vec<_>>();
    let [left, right] = commands.as_slice() else {
        bail!("view <left> <right>");
    };
    let state = Arc::new(Mutex::new(State::default()));

    // Threads
    let left_state = state.clone();
    let left_thread = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(1000));
            let Ok(mut lock) = left_state.lock() else {
                return;
            };
            lock.left.push_str("left\n");
        }
    });
    let right_state = state.clone();
    let right_thread = thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(1000));
            let Ok(mut lock) = right_state.lock() else {
                return;
            };
            lock.right.push_str("left\n");
        }
    });

    // Logic, these threads block
    left_thread.join().unwrap();
    right_thread.join().unwrap();
    
    loop {
        terminal.draw(|f| screen(f, &state))?;

        if let Event::Key(key) = event::read()? {
            if let KeyCode::Char('q') = key.code {
                return Ok(());
            }
        }
    }
}

fn screen<B: Backend>(f: &mut Frame<B>, state: &Arc<Mutex<State>>) {
    let Ok(lock) = state.lock() else {
        return;
    };

    // Declare layout
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)].as_ref())
        .split(f.size());

    // Draw left screen
    let block = Block::default().title(lock.left.clone());
    f.render_widget(block, chunks[0]);

    // Draw right screen
    let block = Block::default().title(lock.right.clone());
    f.render_widget(block, chunks[1]);
}
