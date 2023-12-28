use std::fmt::Display;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Features {
    crate_name: String,
    // file_path: String,
    features: Vec<String>,
}

impl Features {
    pub fn new(crate_name: String, features: Vec<String>) -> Self {
        Features {
            crate_name,
            features,
        }
    }
}

impl Display for Features {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "\nFDelimiter::{{{}}}::FDelimiter\n",
            serde_json::to_string(&self).expect("Fatal, serialize fails")
        )
    }
}

#[test]
fn test() {
    let f = Features::new("Test".into(), vec!["Test1".into(), "Test2".into()]);

    println!("{f}");
}
