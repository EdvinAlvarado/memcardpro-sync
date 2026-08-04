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

use memcardpro_sync::*;
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
    for savefile in find_mcds(src, conn)? {
        savefile.write_to_saves(des.as_ref())?;
    }
    Ok(())
}

/// Finds all mcd files in src path that are named by gameid.
/// it ignores any directories that contain `MemoryCard` in their name, as those are likely not
/// relevant to the search. !todo add supprt for processing `MemoryCard` directories, as they may
/// contain relevant mcd files.
fn find_mcds<P: AsRef<Path>>(src: P, conn: &Connection) -> Result<Vec<SaveFile>> {
    let ps1_dir = src.as_ref().to_path_buf();

    let mut mcd_files = Vec::new();
    for entry in fs::read_dir(ps1_dir)? {
        let entry = entry?;
        if entry.path().is_dir() && !entry.file_name().to_string_lossy().contains("MemoryCard") {
            for file in fs::read_dir(entry.path())? {
                let file = file?;
                if file.path().is_file() {
                    let savefile = SaveFile::from_memcardpro(file.path(), conn)?;
                    mcd_files.push(savefile);
                }
            }
        }
    }
    Ok(mcd_files)
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

    #[test]
    fn parse_srm_name_test() {
        let srm_name = "Front Mission 3 (USA)_1.srm";
        let parsed = parse_srm_name(srm_name).unwrap();
        assert_eq!(parsed.title, "Front Mission 3");
        assert_eq!(parsed.region, "(USA)");
        assert_eq!(parsed.savenum, "1");
    }
}
