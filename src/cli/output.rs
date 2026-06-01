//! Output formatting — human-readable or JSON.
//! All CLI subcommands use these helpers so format logic stays in one place.

use serde::Serialize;

pub enum Format {
    Human,
    Json,
}

impl Format {
    pub fn from_flag(json: bool) -> Self {
        if json { Format::Json } else { Format::Human }
    }

    /// Print a serializable value as JSON or via the human closure.
    pub fn print<T, F>(&self, value: &T, human: F)
    where
        T: Serialize,
        F: FnOnce(&T),
    {
        match self {
            Format::Json => println!("{}", serde_json::to_string_pretty(value).unwrap()),
            Format::Human => human(value),
        }
    }
}
