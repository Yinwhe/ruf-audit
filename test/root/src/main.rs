#![feature(thread_local)]

fn main() {
    println!("Hello, this is root!");
    dep1::dep1_hello();
    dep2::dep2_hello();
    dep2::dep3_hello();
}
