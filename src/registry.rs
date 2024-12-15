use color_eyre::eyre::{bail, OptionExt, Result};
use fs_err as fs;
use sonic_rs::JsonValueTrait;

use crate::json_helpers::get_two_pointers;

pub fn get_rev_from_registry(flake_id: &str) -> Result<String> {
    let contents = fs::read(dirs::config_dir().unwrap().join("nix/registry.json"))?;

    let (flakes, version) = get_two_pointers(&*contents, ["flakes"], ["version"])?;

    match version.as_u64() {
        Some(2) => {}
        Some(num) => bail!("Unsupported version {num}"),
        _ => bail!("Invalid registry"),
    }

    for flake in sonic_rs::to_array_iter(flakes.as_raw_str()) {
        let flake = flake?;

        let (exact, type_) = get_two_pointers(flake.as_raw_str(), ["exact"], ["from", "type"])?;
        if exact.as_bool() != Some(true) || type_.as_str() != Some("indirect") {
            continue;
        }

        let id = sonic_rs::get(flake.as_raw_str(), ["from", "id"])?;
        let rev = sonic_rs::get(flake.as_raw_str(), ["to", "rev"])?;
        if id.as_str() == Some(flake_id) {
            return Ok(rev.as_str().ok_or_eyre("Invalid registry")?.to_owned());
        }
    }

    bail!("No {flake_id} in registry")
}
