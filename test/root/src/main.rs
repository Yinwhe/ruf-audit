#![feature(thread_local)]
#![cfg_attr(feature = "hello", feature(no_std))]

fn main() {
    println!("Hello, this is root!");
    dep1::dep1_hello();
    dep2::dep2_hello();
    dep2::dep3_hello();
}
