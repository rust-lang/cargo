io = IO.popen("rustc libs/hammer.rs/src/hammer.rs --out-dir libs/hammer.rs/target --crate-type lib")

Process.wait2(io.pid)
