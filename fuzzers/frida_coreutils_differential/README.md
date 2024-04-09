# Differential Fuzzing on Coreutils
## Requirements:
- A [rust toolchain](https://www.rust-lang.org/tools/install)
- `clang` (tested with version `14.0.0-1ubuntu1.1`)

## Build
2. Build coreutils using the instrumented compiler
   - > The experiments were initially run with coreutils 9.5:
   1. Download the archive: `wget http://ftp.gnu.org/gnu/coreutils/coreutils-9.5.tar.gz` (or another version of your choosing from [ftp.gnu.org/gnu/coreutils](http://ftp.gnu.org/gnu/coreutils))
   2.  Extract the archive: `tar xf coreutils-9.5.tar.gz`
   3.  Create output directory: `mkdir coreutils-bin`
   4.  Move to the directory: `cd coreutils-bin`
   5.  Configure the project using the instrumented compiler: `CC="$(realpath ../target/release/libafl_cc)" ../coreutils-9.5/configure`
   6.  Build the binaries: `make`
3. Build the fuzzer: `cargo build --release --bin fuzzer`