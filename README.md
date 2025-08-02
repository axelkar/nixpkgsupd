# nixpkgsupd

Updates Nix flakes found in Nix's garbage collector roots and provides tools to
manage the garbage collector roots.

This is most useful for `nixos-unstable` users to reduce Nix store size because packages update often.

## Usage

```console
$ nixpkgsupd --target ~/.nixos-config'#'nixpkgs list
nixpkgs target: nixos-unstable last updated a month ago
/home/axel/dev/example (direnv): nixos-25.05 1f08a4df998e21f4e8be8fb6fbf61d11a1a5076a last updated 3 days ago

$ nixpkgsupd --target ~/.nixos-config'#'nixpkgs update
Note: This is a dry run. To modify files and run commands, run again with --allow-write
nixpkgs target: nixos-unstable last updated a month ago

/home/axel/dev/example (direnv): nixos-25.05 1f08a4df998e21f4e8be8fb6fbf61d11a1a5076a last updated 3 days ago
 {
   description = "A basic flake with a shell";
-  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.05";
+  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
   inputs.flake-utils.url = "github:numtide/flake-utils";

   outputs = { nixpkgs, flake-utils, ... }:
(1/6) [a,n,e,sh,up,dg,lock,direnv,commit,?]
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

Make a GitHub [pull request](https://github.com/axelkar/nixoptupd/pulls).

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
