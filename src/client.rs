use std::{error::Error, io::{self, ErrorKind, Read, Write}, mem::size_of_val, net::TcpStream, process::exit, sync::{Arc, Mutex, MutexGuard, mpsc}, thread::{self, sleep}, time::{Duration}};
use chrono::{DateTime, Local, Utc};
use crossterm::{self, event::{KeyCode, read, Event}};
use tui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use unicode_width::UnicodeWidthStr;

use crate::{structs::{Msg, ConnectionRequest, MsgType, MessageWrapper}, config::Config};

pub const COLOR_INFO: Color = Color::LightBlue;
pub const COLOR_ERR: Color = Color::LightRed;
pub const COLORS: [&str; 16] = ["black", "red","green", "yellow", "blue", "magenta", "cyan", "gray", "darkgray", "lightred", "lightgreen", "lightyellow", "lightblue", "lightmagenta", "lightcyan", "white"];

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

pub struct Parsed {
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

fn request_connection(username: &str, room: String, stream: &mut TcpStream) -> Result<(), Box<dyn Error>> {
    let request = ConnectionRequest {
        username: username.to_string(),
        room: room
    };

    let serialized_request = serde_json::to_string(&request)?;

    let request_string = serde_json::to_string(&MessageWrapper {
        msg_type: MsgType::ConnectionRequest,
        msg: serialized_request
    })?;

    stream.write_all(&size_of_val(request_string.as_bytes()).to_be_bytes())?;
    stream.write_all(request_string.as_bytes())?;
    return Ok(());
}

// TODO implement config files
pub fn start(addr: String, username: String, config: Config) -> Result<(), Box<dyn Error>> {
	ctrlc::set_handler(move || {
		println!("Exiting...");
		quit();
	}).expect("Failed to set ctrlc handler");

    let username: &str = &username;

	let client = Arc::new(Mutex::new(Client::new(username.to_string())));

	let mut stream = TcpStream::connect(addr).expect("Failed to connect to server.");
	stream.set_nonblocking(true)?;

	let stdout = io::stdout();
	
    let backend = CrosstermBackend::new(stdout);
	let mut terminal = Terminal::new(backend)?;

	// let events = Events::new();

	let app = Arc::new(Mutex::new(App::default()));
	
    let (tx, rx) = mpsc::channel::<Msg>();
    let (tx_i, rx_i) = mpsc::channel::<Msg>();

    let shared_tx = Arc::new(Mutex::new(tx));


    // TODO this should default to _default, but otherwise should use last joined room or specified in :open command
    request_connection(username, String::from("_default"), &mut stream)?;

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

        let mut app_t = app.lock().unwrap();
        
        // Handle input
        match read()? {
            Event::Key(event) => {
                match event.code {
                    KeyCode::Char('c') => break,
                    KeyCode::Enter => {
                        let msg = app_t.input.drain(..).collect();
                        let parse = parse_message(msg, shared_tx.lock().unwrap(), client.lock().unwrap());
                        if parse.should_print {
                            let cl = client.lock().unwrap();
                            app_t.messages.push(Msg{sender: (&cl.name).to_string(), content: parse.content, color: parse.color, timestamp: Utc::now()});
                        }
                    },
                    KeyCode::Backspace => {
                        app_t.input.pop();
                    },
                    KeyCode::Char(c) => {
                        app_t.input.push(c);
                    }
                    _ => {}
                }
            },
            Event::Mouse(_event) => {},
            Event::Resize(_w, _h) => {}
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
            "help" => {
            if cmd.len() != 2 {
                Parsed {
                    should_print: true,
                    content: format!("Available commands: /help, /nick <nickname>, /color <color>"),
                    color: COLOR_INFO
                }
            } else {
                match cmd[1] {
                    "nick" => {
                        Parsed {
                            should_print: true,
                            content: format!("Usage of nick: /nick <nickname>"),
                            color: COLOR_INFO
                        }
                    },
                    "color" => {
                        Parsed {
                            should_print: true,
                            content: format!("Available colors: {}", COLORS.join(", ")),
                            color: COLOR_INFO
                        }
                    },
                    _ => {
                        Parsed {
                            should_print: true,
                            content: format!("Available commands: /help, /nick <nickname>, /color <color>"),
                            color: COLOR_INFO
                        }
                    }
                }
            }
        }
        "nick" => {
            if cmd.len() != 2 {
                return Parsed {
                    should_print: true,
                    content: format!("Incorrect usage of command! /nick <name>"),
                    color: COLOR_ERR
                }
            }
            let arg = cmd[1];
            client.name = String::from(arg);
            return Parsed {
                should_print: true,
                content: format!("Changed name to: {}", &client.name),
                color: COLOR_INFO
            }
        }
        "info" => {
            return Parsed {
                should_print: true,
                content: (*client.name).to_string(),
                color: COLOR_INFO
            }
        }
        "open" => todo!(),
        "remote-color" => {
            if cmd.len() != 2 {
                return Parsed {
                    should_print: true,
                    content: format!("Incorrect usage of command! /remote-color <color>"),
                    color: COLOR_ERR
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
                    color: COLOR_ERR
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
                color: COLOR_ERR
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
        "black"        => {Ok(Color::Black)}
        "red"          => {Ok(Color::Red)}
        "green"        => {Ok(Color::Green)}
        "yellow"       => {Ok(Color::Yellow)}
        "blue"         => {Ok(Color::Blue)}
        "magenta"      => {Ok(Color::Magenta)}
        "cyan"         => {Ok(Color::Cyan)}
        "gray"         => {Ok(Color::Gray)}
        "darkgray"     => {Ok(Color::DarkGray)}
        "lightred"     => {Ok(Color::LightRed)}
        "lightgreen"   => {Ok(Color::LightGreen)}
        "lightyellow"  => {Ok(Color::LightYellow)}
        "lightblue"    => {Ok(Color::LightBlue)}
        "lightmagenta" => {Ok(Color::LightMagenta)}
        "lightcyan"    => {Ok(Color::LightCyan)}
        "white"        => {Ok(Color::White)}
        _ => {
            Err(String::from("No such color. Try /help colors"))
        }
    }
}

fn quit() {
    println!("\x1B[2J\x1B[1;1H");
    exit(0);
}
