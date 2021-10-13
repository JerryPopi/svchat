use std::time::SystemTime;

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use tui::style::Color;

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
