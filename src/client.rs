#![allow(unused_imports)]

use std::{convert::TryInto, error::Error, io::{self, ErrorKind, Read, Write}, net::TcpStream, process::exit, sync::{Arc, Mutex, MutexGuard, mpsc}, thread::{self, sleep}, time::Duration};
use termion::{event::Key, input::{MouseTerminal, TermRead}, raw::IntoRawMode, screen::AlternateScreen};
use tui::{
    backend::TermionBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Span, Spans, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use unicode_width::UnicodeWidthStr;

use crate::{message::Msg, events::*};

pub struct Client {
	pub name: Vec<char>
}

impl Client {
	fn new(username: String) -> Client{
		return Client {
			name: username.chars().collect()
		}
	}
}

struct App {
	input: String,
	messages: Vec<String>
}

impl Default for App {
	fn default() -> App {
		App {
			input: String::new(),
			messages: Vec::new()
		}
	}
}

struct Parsed {
    should_print: bool,
    content: String
}

impl Default for Parsed {
    fn default() -> Parsed {
        Parsed {
            should_print: false,
            content: String::new()
        }
    }
}

pub fn start(addr: String, username: String) -> Result<(), Box<dyn Error>> {
	ctrlc::set_handler(move || {
		println!("Exiting...");
		quit();
	}).expect("Failed to set ctrlc handler");

    #[allow(unused_variables)]
	let client = Arc::new(Mutex::new(Client::new(username)));

	let mut stream = TcpStream::connect(addr).expect("Failed to connect to server.");
	stream.set_nonblocking(true)?;

	let stdout = io::stdout().into_raw_mode()?;
	let stdout = MouseTerminal::from(stdout);
	let stdout = AlternateScreen::from(stdout);
	let backend = TermionBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;

	let events = Events::new();

	let app = Arc::new(Mutex::new(App::default()));
	
    let (tx, rx) = mpsc::channel::<Msg>();

    let shared_tx = Arc::new(Mutex::new(tx));

    thread::spawn(move || loop {
		let mut buf_sz = [0; std::mem::size_of::<usize>()];
		match stream.read_exact(&mut buf_sz) {
			Ok(_) => {
				let json_sz = usize::from_be_bytes(buf_sz);
                let mut msg_buf: Vec<u8> = vec![0, json_sz.try_into().unwrap()];
                match stream.read_exact(&mut msg_buf){
                    Ok(_) => {
                        let msg: Msg = serde_json::from_str(String::from_utf8(msg_buf).unwrap().as_str()).unwrap();
                    },
                    Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
                    Err(_) => {
                        println!("Error while reading json data from server");
                        break;
                    }
                }
			},
            Err(ref err) if err.kind() == ErrorKind::WouldBlock => (),
    		Err(_) => {
				println!("Connection with server was severed!");
				break;
			}
		}

        match rx.try_recv() {
            Ok(msg) => {
                let outbound_json = serde_json::to_string(&msg).unwrap();
				let outbound = outbound_json.as_bytes();
                stream.write_all(&outbound).unwrap();
            },
            Err(mpsc::TryRecvError::Empty) => (),
            Err(mpsc::TryRecvError::Disconnected) => break
        }
        sleep(Duration::from_millis(100));
	});

    terminal.clear().unwrap();

    loop {
		terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(0)
                .constraints(
                    [
                        // Constraint::Min(1),
                        Constraint::Percentage(90),
                        Constraint::Percentage(10),
                    ]
                    .as_ref(),
                )
                .split(f.size());

            let app_t = app.lock().unwrap();
            let input = Paragraph::new(app_t.input.as_ref())
                .style(Style::default())
                .block(Block::default().borders(Borders::NONE));

            f.render_widget(input, chunks[1]);
            f.set_cursor(
                // Put cursor past the end of the input text
                chunks[1].x + app_t.input.width() as u16, //+ 1,
                // Move one line down, from the border to the input line
                chunks[1].y //+ 1,
            );
            let messages: Vec<ListItem> = app_t
                .messages
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let content = vec![Spans::from(Span::raw(format!("{}: {}", i, m)))];
                    ListItem::new(content)
                })
                .collect();
            let messages =
                List::new(messages).block(Block::default().borders(Borders::NONE)); //.title("Messages"));
            f.render_widget(messages, chunks[0]);
            drop(app_t);
        })?;

        // Handle input
        if let Event::Input(input) = events.next()? {
            let mut app_t = app.lock().unwrap();
            match input {
                Key::Ctrl('c') => {
                    break;
                }
                Key::Char('\n') => {
                    let msg = app_t.input.drain(..).collect();
                    let parse = parse_message(msg, shared_tx.lock().unwrap());
                    if parse.should_print {
                        app_t.messages.push(parse.content);
                    }
                    
                },
                Key::Backspace => {
                    app_t.input.pop();
                },
                Key::Char(c) => {
                    app_t.input.push(c);
                },
                _ => {}
            }
        }
	}
    terminal.clear().unwrap();
    drop(terminal);
    Ok(())
}

fn parse_message(msg: String, tx: MutexGuard<mpsc::Sender<Msg>>) -> Parsed {
    if msg.starts_with('/') {
        match msg[1..].as_ref() {
            "rename" => {
                Parsed::default()
            }
            _ => {
                Parsed {
                    should_print: true,
                    content: String::from("Unknown command. Try /help")
                }
            }
        }
    } else {
        Parsed::default()
    }
}

fn quit() {
    println!("\x1B[2J\x1B[1;1H");
    exit(0);
}
