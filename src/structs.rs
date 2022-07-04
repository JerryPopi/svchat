use std::{net::{TcpStream, SocketAddr}, collections::HashMap};

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use tui::style::Color;

pub struct Connection {
	pub(crate) stream: TcpStream,
	pub(crate) username: String
}

#[derive(Serialize, Deserialize, Debug)]
pub enum MsgType {
	ConnectionRequest,
	Message,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MessageWrapper {
	pub msg_type: MsgType,
	pub msg: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct Msg {
	pub content: String,
	pub sender: String,
	pub color: Color,
	pub timestamp: DateTime<Utc>
}	

impl Default for Msg {
	fn default() -> Msg {
		Msg {
			content: String::new(),
			sender: String::new(),
			color: Color::White,
			timestamp: Utc::now()
		}
	}
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ConnectionRequest {
	pub username: String,
	pub room: String
}

#[derive(Clone)]
pub struct Room {
	pub clients: Vec<SocketAddr>,
	//todo room options
}

impl Room {
	pub fn add_user(&mut self, user: SocketAddr) {
		self.clients.push(user);
	}
}

#[derive(Clone)]
pub struct RoomList<'a> {
	pub rooms: &'a HashMap<String, Room>
}

impl Default for RoomList<'a> {
	fn default() -> Self {
        Self { rooms: Default::default() }
    }
}

