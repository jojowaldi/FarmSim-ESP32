fn main() {
  // Make sure linkall.x is the last linker script
  println!("cargo:rustc-link-arg=-Tlinkall.x");
}

