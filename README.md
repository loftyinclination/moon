Use `cargo +nightly run -Z build-std=std,panic_abort` to build and run.
`cargo +nightly bloat -Z build-std=std,panic_abort --symbols-section .rodata -n 0` command shows that the rodata section size is large, but apparently empty of symbols.
