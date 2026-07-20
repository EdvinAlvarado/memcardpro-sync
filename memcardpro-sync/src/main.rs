use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufRead, BufReader},
    net::Ipv4Addr,
    path::Path,
    path::PathBuf,
    process::Command,
};

use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};
use main_error::MainError;
use sqlite::Connection;

mod strings;
use crate::strings::*;
mod database;
use crate::database::*;

/// Memcard-Sync Program
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    /// memcard-sync
    #[arg(short, long)]
    db: PathBuf,
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Runs backup
    Convert {
        /// Path to memcard backup
        #[arg(short, long)]
        input: PathBuf,
        /// Directory where to restore archive
        #[arg(short, long)]
        output: PathBuf,
        /// Whether to convert from mccardpro or to
        #[arg(short, long)]
        reverse: bool,
    },
}

fn main() -> Result<(), MainError> {
    let cli = Cli::parse();

    let conn = sqlite::open(cli.db)?;

    let _cmd_result = match cli.command {
        Some(Commands::Convert {
            input,
            output,
            reverse,
        }) => convert(input, &conn, output),
        None => Err(anyhow!("no command")),
    };
    _cmd_result?;
    Ok(())
}

fn convert<P: AsRef<Path>>(src: P, conn: &Connection, des: P) -> Result<()> {
    for mcd_path in find_mcds(src)? {
        let srm = mcd_to_srm(mcd_path.as_path(), conn)?;
        let mut srm_path = des.as_ref().to_path_buf();
        srm_path.push(srm.as_ref());

        fs::copy(&mcd_path, &srm_path)?;
        println!("Copied: {}\t->\t{}", mcd_path.display(), srm_path.display());
    }
    Ok(())
}

/// Finds all mcd files in src path that are named by gameid.
fn find_mcds<P: AsRef<Path>>(src: P) -> Result<Vec<PathBuf>> {
    let mut ps1_dir = src.as_ref().to_path_buf();
    ps1_dir.push("PS1");

    let mut mcd_files = Vec::new();
    for entry in fs::read_dir(ps1_dir)? {
        let entry = entry?;
        if entry.path().is_dir() && !entry.file_name().to_string_lossy().contains("MemoryCard") {
            for file in fs::read_dir(entry.path())? {
                let file = file?;
                if file.path().is_file() {
                    mcd_files.push(file.path());
                }
            }
        }
    }
    mcd_files.sort();

    Ok(mcd_files)
}

/// Creates emulator-friendly save file name from memcardpro save file name.
fn mcd_to_srm<P: AsRef<Path> + Clone>(mcd_path: P, conn: &Connection) -> Result<Box<str>> {
    let info = GameInfo::from_path(mcd_path.as_ref(), conn)?
        .ok_or(anyhow!("mcd path doesn't seem to match a memcardpro path"))?;
    let region = info.get_region()?;

    let savenum = mcd_path
        .as_ref()
        .file_name()
        .unwrap()
        .to_string_lossy()
        .chars()
        .nth_back(4)
        .and_then(|c| c.to_digit(10))
        .map(|n| format!("_{}", n))
        .ok_or(anyhow!(
            "mcd does not follow memcardpro naming convention: {:?}",
            mcd_path.as_ref()
        ))?;

    let title = match region.is_empty() {
        true => format!("{}", info.title.to_string()),
        false => format!(
            "{} {}",
            capitalize_first_letters(info.title.to_lowercase()),
            region
        ),
    };

    let srm = title + savenum.as_str() + ".mcd";
    Ok(srm.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_info() {
        let conn =
            sqlite::open("/home/edvin/Projects/memcardpro-sync/data/ps1_games.sqlite3").unwrap();
        for mcd_path in
            find_mcds("/home/edvin/Insync/silryk31@gmail.com/Google Drive/MCPro2_28372fec8c50/")
                .unwrap()
        {
            println!(
                "Found {:?}",
                GameInfo::from_path(mcd_path, &conn).unwrap().unwrap()
            );
        }
    }

    #[test]
    fn make_srm_name() {
        let conn =
            sqlite::open("/home/edvin/Projects/memcardpro-sync/data/ps1_games.sqlite3").unwrap();
        let mut res = HashMap::new();
        for mcd_path in
            find_mcds("/home/edvin/Insync/silryk31@gmail.com/Google Drive/MCPro2_28372fec8c50/")
                .unwrap()
        {
            let srm = mcd_to_srm(&mcd_path, &conn).unwrap();
            let mcd = mcd_path
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_owned()
                .to_string();
            println!("{}\t-->\t{}", mcd, srm);
            res.insert(mcd, srm);
        }
        assert_eq!(
            res.get("SCUS-94423-1.mcd").map(|s| s.as_ref()),
            Some("Ape Escape (USA)_1.mcd")
        );
        assert_eq!(
            res.get("SLUS-01011-1.mcd").map(|s| s.as_ref()),
            Some("Front Mission 3 (USA)_1.mcd")
        );
        assert_eq!(
            res.get("TLWL-94221-1.mcd").map(|s| s.as_ref()),
            Some("Final Fantasy Tactics (USA) (patched TLWotL v1.02)_1.mcd")
        );
    }
}
