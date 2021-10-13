use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone)]
pub struct Msg {
	content: String,
	sender: String
}

impl Default for Msg {
	fn default() -> Msg {
		Msg {
			content: String::new(),
			sender: String::new()
		}
	}
}
