#![allow(unused_imports)]

use std::{convert::TryInto, error::Error, io::{self, ErrorKind, Read, Write}, mem::size_of_val, net::TcpStream, process::exit, sync::{Arc, Mutex, MutexGuard, mpsc}, thread::{self, sleep}, time::{Duration, SystemTime, UNIX_EPOCH}};
use chrono::{DateTime, Local, Utc};
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

const INFO_COLOR: Color = Color::LightBlue;
const ERR_COLOR: Color = Color::LightRed;

pub struct Client {
	pub name: String,
    pub local_color: Color,
    pub remote_color: Color
}

impl Client {
	fn new(username: String) -> Client{
		return Client {
			name: username,
            local_color: Color::White,
            remote_color: Color::White
		}
	}
}

struct App {
	input: String,
	messages: Vec<Msg>
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
    content: String,
    color: Color
}

impl Default for Parsed {
    fn default() -> Parsed {
        Parsed {
            should_print: false,
            content: String::new(),
            color: Color::White
        }
    }
}

pub fn start(addr: String, username: String) -> Result<(), Box<dyn Error>> {
	ctrlc::set_handler(move || {
		println!("Exiting...");
		quit();
	}).expect("Failed to set ctrlc handler");

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
    let (tx_i, rx_i) = mpsc::channel::<Msg>();

    let shared_tx = Arc::new(Mutex::new(tx));

    thread::spawn(move || loop {
		let mut buf_sz = [0; std::mem::size_of::<usize>()];
		match stream.read_exact(&mut buf_sz) {
			Ok(_) => {
				let json_sz = usize::from_be_bytes(buf_sz);
                let mut msg_buf: Vec<u8> = vec![0; json_sz];
                match stream.read_exact(&mut msg_buf){
                    Ok(_) => {
                        let msg: Msg = serde_json::from_str(String::from_utf8(msg_buf).unwrap().as_str()).unwrap();
                        tx_i.send(msg).unwrap();
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

                stream.write_all(&size_of_val(outbound).to_be_bytes()).unwrap();
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
            let mut app_t = app.lock().unwrap();
            match rx_i.try_recv() {
                Ok(msg) => {
                    if msg.sender != client.lock().unwrap().name {
                        app_t.messages.push(msg);
                    }
                }
                Err(mpsc::TryRecvError::Empty) => (),
                Err(mpsc::TryRecvError::Disconnected) => return
            }
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
                .map(|(_i, m)| {
                    let style = Style::default().fg(m.color);
                    let local: DateTime<Local> = DateTime::from(m.timestamp);
                    let time = local.format("%H:%M:%S").to_string();
                    let content = vec![Spans::from(Span::styled(format!("{} {}: {}", time,  m.sender, m.content), style))];
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
                    let parse = parse_message(msg, shared_tx.lock().unwrap(), client.lock().unwrap());
                    if parse.should_print {
                        let cl = client.lock().unwrap();
                        app_t.messages.push(Msg{sender: (&cl.name).to_string(), content: parse.content, color: parse.color, timestamp: Utc::now()});
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

fn parse_message(msg: String, tx: MutexGuard<mpsc::Sender<Msg>>, mut client: MutexGuard<Client>) -> Parsed {
    let msg = msg.trim().to_string();
    if msg.starts_with('/') {
        let msg: &str = msg[1..].as_ref();
        let cmd = msg.split(' ').collect::<Vec<&str>>();

        match cmd[0] {
            "rename" => {
                if cmd.len() != 2 {
                    return Parsed {
                        should_print: true,
                        content: String::from("Incorrect usage of command! /rename <name>"),
                        color: ERR_COLOR
                    }
                }
                let arg = cmd[1];
                client.name = String::from(arg);
                return Parsed {
                    should_print: true,
                    content: String::from("Changed name to: ".to_owned() + &client.name),
                    color: INFO_COLOR
                }
            }
            "info" => {
                return Parsed {
                    should_print: true,
                    content: (*client.name).to_string(),
                    color: INFO_COLOR
                }
            }
            "open" => todo!(),
            "remote-color" => {
                if cmd.len() != 2 {
                    return Parsed {
                        should_print: true,
                        content: String::from("Incorrect usage of command! /remote-color <color>"),
                        color: ERR_COLOR
                    }
                }
                let arg = cmd[1];
                let out_str: String;
                let mut color = Color::White;
                match color_from_name(arg) {
                    Ok(_color) => {
                        out_str = String::from("Changed color to ".to_owned() + arg);
                        color = _color;
                    }
                    Err(e) => {
                        out_str = e;
                    }
                }
                client.remote_color = color;
                return Parsed {
                    should_print: true,
                    content: out_str,
                    color,
                }
            }
            "local-color" => {
                if cmd.len() != 2 {
                    return Parsed {
                        should_print: true,
                        content: String::from("Incorrect usage of command! /local-color <color>"),
                        color: ERR_COLOR
                    }
                }
                let arg = cmd[1];
                let out_str: String;
                let mut color = Color::White;
                match color_from_name(arg) {
                    Ok(_color) => {
                        out_str = String::from("Changed color to ".to_owned() + arg);
                        color = _color;
                    }
                    Err(e) => {
                        out_str = e;
                    }
                }
                client.local_color = color;
                return Parsed {
                    should_print: true,
                    content: out_str,
                    color,
                }
            }
            _ => {
                Parsed {
                    should_print: true,
                    content: String::from("Unknown command. Try /help"),
                    color: ERR_COLOR
                }
            }
        }
    } else {
        tx.send(Msg{content: msg.clone().to_owned(), 
            sender: (*client.name).to_string().to_owned(), 
            color: client.remote_color, 
            timestamp: Utc::now()})
            .unwrap();
        Parsed {
            should_print: true,
            content: msg,
            color: client.local_color
        }
    }
}

fn color_from_name(color: &str) -> Result<Color, String> {
    match color.to_lowercase().as_str() {
        "black" => {Ok(Color::Black)}
        "red" => {Ok(Color::Red)}
        "green" => {Ok(Color::Green)}
        "yellow" => {Ok(Color::Yellow)}
        "blue" => {Ok(Color::Blue)}
        "magenta" => {Ok(Color::Magenta)}
        "cyan" => {Ok(Color::Cyan)}
        "gray" => {Ok(Color::Gray)}
        "darkgray" => {Ok(Color::DarkGray)}
        "lightred" => {Ok(Color::LightRed)}
        "lightgreen" => {Ok(Color::LightGreen)}
        "lightyellow" => {Ok(Color::LightYellow)}
        "lightblue" => {Ok(Color::LightBlue)}
        "lightmagenta" => {Ok(Color::LightMagenta)}
        "lightcyan" => {Ok(Color::LightCyan)}
        "white" => {Ok(Color::White)}
        _ => {
            Err(String::from("No such color. Try /help colors"))
        }
    }
}

fn quit() {
    println!("\x1B[2J\x1B[1;1H");
    exit(0);
}
