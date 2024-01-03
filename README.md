# ruf-audit
Audit ruf usage in crates

# Usage
Before buid, please add needed env:
```bash
rustup toolchain install nightly-2023-12-12
rustup component add rustc-dev llvm-tools
```

And then you can build with:
```bash
cargo build
```

And please use cli rather than `cargo run` to use this tools:
```bash
ruf_audit # Please run this cli in the root of crates
```

TODO:
- [x] Scan and extract rufs
- [ ] Analyze and choose suitable deps
