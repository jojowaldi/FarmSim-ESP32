fn main() {
  println!("cargo:rustc-link-arg=-Tdefmt.x");
  // Make sure linkall.x is the last linker script
  println!("cargo:rustc-link-arg=-Tlinkall.x");
}

