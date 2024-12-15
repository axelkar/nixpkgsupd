# nixpkgsupd

Updates Nix flakes in Nix's garbage collector roots.

This is most useful for Nixpkgs unstable users to reduce Nix store size, since packages update often.

## Usage

```console
$ nixpkgsupd --allow-write
Global rev: 5d67ea6b4b63378b9c13be21e2ec9d1afc921713
/home/axel/example: b054d170785fceabcf4fef592dbb82914d78f03c
 {
   inputs = {
-    nixpkgs.url = github:NixOS/nixpkgs;
+    nixpkgs.url = "github:NixOS/nixpkgs/5d67ea6b4b63378b9c13be21e2ec9d1afc921713";
     rust-overlay.url = "github:oxalica/rust-overlay";
     rust-overlay.inputs.nixpkgs.follows = "nixpkgs";
   };
Write this? [y,n,e,?] y
warning: Git tree '/home/axel/example' is dirty
warning: updating lock file '/home/axel/example/flake.lock':
• Updated input 'nixpkgs':
    'github:NixOS/nixpkgs/b054d170785fceabcf4fef592dbb82914d78f03c?narHash=sha256-V99zbdrr5zgjdvtKJ2AUGxLYGlchkzWLg7KB202dj7k%3D' (2024-05-20)
  → 'github:NixOS/nixpkgs/5d67ea6b4b63378b9c13be21e2ec9d1afc921713?narHash=sha256-Pj39hSoUA86ZePPF/UXiYHHM7hMIkios8TYG29kQT4g%3D' (2024-12-11)
warning: Git tree '/home/axel/example' is dirty
Update direnv? [y,n] y
direnv: loading ~/example/.envrc
direnv: using flake
warning: Git tree '/home/axel/example' is dirty
warning: Git tree '/home/axel/example' is dirty
direnv: nix-direnv: Renewed cache
Commit flake.nix and flake.lock into Git? [y,n] (No commits yet) (Stage is dirty) n
```

## Development

0. Have Linux or MacOS

1. Install [Nix](https://nixos.org/download#download-nix)

2. Run the command `nix develop` in a shell.

   This creates a `bash` subshell with all the dependencies.

3. Run `cargo` commands as you like.

   i.e. `cargo build`, `cargo run`, `cargo clippy`, etc.

## Contributing patches

Please first make sure that you have not introduced any regressions and format the code by running the following commands at the repository root.
```sh
cargo fmt
cargo clippy
cargo test
```

Make a GitHub [pull request](https://github.com/axelkar/nixoptudp/pulls)

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
