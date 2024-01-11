#![feature(imported_main)]

pub fn dep2_hello() {
    println!("Hello, this is dep2 !")
}

pub use dep3::dep3_hello;