use anyhow::{anyhow, Result};
use cargo_metadata::MetadataCommand;
use clap::Parser;
use std::{borrow::Cow, ffi::OsStr, fs, path::PathBuf, process::Command};

#[derive(Parser, Debug)]
struct Args {
    /// Path to Cargo.toml
    #[arg(long)]
    manifest_path: PathBuf,
    /// Build with the specified profile
    #[arg(long, default_value = "release")]
    profile: String,
    /// Directory to copy wasm files to
    #[arg(long)]
    out_dir: Option<std::path::PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let strip_has_abs_paths = run("soroban_eth_abi", &args, Strip::Yes)?;
    let nostrip_has_abs_paths = run("soroban_eth_abi", &args, Strip::No)?;

    assert_eq!(strip_has_abs_paths, false);
    assert_eq!(nostrip_has_abs_paths, true);

    Ok(())
}

#[derive(Eq, PartialEq)]
enum Strip { Yes, No }

fn run(
    contract_name: &str,
    args: &Args,
    strip: Strip,
) -> Result<bool> {
    let stellar_cli = "../stellar-cli/target/debug/soroban";
    let mut cmd = Command::new(stellar_cli);
    cmd.arg("contract");
    cmd.arg("build");
    cmd.arg(format!(
        "--manifest-path={}",
        args.manifest_path.to_string_lossy()
    ));
    cmd.arg(format!("--profile={}", args.profile));

    if let Some(ref out_dir) = args.out_dir {
        cmd.arg(format!("--out-dir={}", out_dir.display()));
    };

    if strip == Strip::No {
        // This will prevent stellar-cli from setting CARGO_BUILD_RUSTFLAGS,
        // and removing absolute paths.
        // See docs for `make_rustflags_to_remap_absolute_paths`.
        cmd.env("RUSTFLAGS", "");
    }

    let cmd_str = print_cmd(&cmd)?;

    let status = cmd.status()?;
    if !status.success() {
        return Err(anyhow!("failed building with stellar: {cmd_str:?}"));
    }

    let manifest_dir = args.manifest_path.parent().expect("path");
    let wasm_path = manifest_dir
        .join("target/wasm32-unknown-unknown/release")
        .join(format!("{contract_name}.wasm"));

    contains_absolute_paths(&wasm_path)
}

fn contains_absolute_paths(wasm: &PathBuf) -> Result<bool> {
    let cargo_home = home::cargo_home()?;
    let registry_prefix = format!("{}/registry/src/", &cargo_home.display());

    let wasm_buf = fs::read(wasm)?;
    let wasm_str = String::from_utf8_lossy(&wasm_buf);

    if wasm_str.contains(&registry_prefix) {
        Ok(true)
    } else {
        Ok(false)
    }
}

fn print_cmd(cmd: &Command) -> Result<String> {
    let mut cmd_str_parts = Vec::<String>::new();
    cmd_str_parts.extend(cmd.get_envs().map(|(key, val)| {
        format!(
            "{}={}",
            key.to_string_lossy(),
            shell_escape::escape(val.unwrap_or_default().to_string_lossy())
        )
    }));
    cmd_str_parts.push("(stellar-cli)".to_string());
    cmd_str_parts.extend(
        cmd.get_args()
            .map(OsStr::to_string_lossy)
            .map(Cow::into_owned),
    );
    let cmd_str = cmd_str_parts.join(" ");
    println!("{cmd_str}");

    Ok(cmd_str.to_string())
}
