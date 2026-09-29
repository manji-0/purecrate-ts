//! `examples/payment` as it is: its types already derive serde, so the WASM
//! build runs the very source the TS package is generated from, with the real
//! serde (the workspace's `check` only sees a stand-in).

fn main() {
    let src = "../../../examples/payment/src/lib.rs";
    println!("cargo:rerun-if-changed={src}");
    let text = std::fs::read_to_string(src).expect("read examples/payment");
    let dest = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("payment.rs");
    std::fs::write(dest, text).expect("write");
}
