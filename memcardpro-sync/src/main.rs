use std::{path::Path, path::PathBuf};

use anyhow::{anyhow, Context, Result};
use clap::{Parser, Subcommand};
use main_error::MainError;
use sqlite::Connection;

use memcardpro_sync::*;
mod database;

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
    for entry in std::fs::read_dir(ps1_dir)? {
        let entry = entry?;
        if entry.path().is_dir() && !entry.file_name().to_string_lossy().contains("MemoryCard") {
            for file in std::fs::read_dir(entry.path())? {
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
    fn memcard_to_saves_test() {
        let conn = sqlite::open("data/ps1_games.sqlite3").unwrap();
        let mcd_files = find_mcds("tests/memcardpro", &conn).unwrap();
        for savefile in mcd_files {
            savefile.write_to_saves(Path::new("tests/saves")).unwrap();
        }
        let count = std::fs::read_dir("tests/saves")
            .unwrap()
            .filter(|entry| entry.as_ref().map(|e| e.path().is_file()).unwrap_or(false))
            .count();
        assert_eq!(count, 2);
    }
}
