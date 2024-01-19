use std::path::Path;

use cargo_lock::{Lockfile, Error};

pub fn parse_from_path(path: impl AsRef<Path>) -> Result<Lockfile, Error> {
    Lockfile::load(path)
}

#[test]
fn test() {
    let lockfile = parse_from_path("/home/ubuntu/Workspaces/ruf-audit/test/root/Cargo.lock").unwrap();
    println!("{:#?}", lockfile.dependency_tree());
}