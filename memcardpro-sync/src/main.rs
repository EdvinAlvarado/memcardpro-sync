use std::{
    collections::HashMap,
    fs::{self, File},
    io::{self, BufRead, BufReader},
    net::Ipv4Addr,
    path::Path,
    path::PathBuf,
    process::Command,
};

use clap::{Parser, Subcommand};
use lazy_static::lazy_static;
use main_error::MainError;
use sqlite::Connection;
use thiserror::Error;

#[derive(Error, Debug)]
enum MemcardError {
    #[error("IO error")]
    CommandError(#[from] io::Error),
    #[error("mcd file doesn't expected naming convention")]
    McdNameError(),
    #[error("No command provided")]
    NoCommand,
}

#[derive(Clone, Debug)]
struct GameInfo {
    code: Box<str>,
    title: Box<str>,
    lang: Box<str>,
}

type MemCardResult<T> = Result<T, MemcardError>;

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
        /// Whether to convert from mcd to srm
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
        None => Err(MemcardError::NoCommand),
    };
    _cmd_result?;
    Ok(())
}

lazy_static! {
    static ref REGION_MAP: HashMap<&'static str, &'static str> = {
        let mut m = HashMap::new();
        m.insert("SLUS", "(USA)");
        m.insert("SCUS", "(USA)");
        m.insert("LPS", "(USA)");
        m.insert("SLES", "(Europe)");
        m.insert("SCES", "(Europe)");
        m.insert("SCED", "(Europe)");
        m.insert("SLPS", "(Japan)");
        m.insert("SCPS", "(Japan)");
        m.insert("SIPS", "(Japan)");
        m.insert("SLMS", "(Japan)");
        m.insert("CPCS", "(Japan)");
        m.insert("SCAJ", "(Japan)");
        m.insert("ESPM", "(Japan)");
        m.insert("SLKA", "(Japan)");
        m.insert("HPS", "(Japan)");
        m
    };
}

fn find_mcds<P: AsRef<Path>>(src: P) -> MemCardResult<Vec<PathBuf>> {
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

fn get_info<S: AsRef<str>>(code: S, conn: &Connection) -> MemCardResult<Option<GameInfo>> {
    let query = "SELECT * FROM ps1 WHERE code = ?";

    let info = conn
        .prepare(query)
        .unwrap()
        .into_iter()
        .bind((1, code.as_ref()))
        .unwrap()
        .map(|row| row.unwrap())
        .map(|row| GameInfo {
            code: row.read::<&str, _>("code").into(),
            title: row.read::<&str, _>("title").into(),
            lang: row.read::<&str, _>("language").into(),
        })
        .nth(0);
    Ok(info)
}

fn mcd_to_srm<P: AsRef<Path>>(mcd_path: P, conn: &Connection, des: P) -> MemCardResult<()> {
    let parent_dir = mcd_path.as_ref().parent().unwrap();
    let code = parent_dir.file_name().unwrap().to_string_lossy();
    let info = get_info(code, conn)?.unwrap();
    let region_code = info.code.split_once('-').unwrap().0;
    let region = REGION_MAP
        .get(region_code)
        .ok_or(MemcardError::McdNameError())?;
    let mcd = mcd_path.as_ref().file_name().unwrap().to_string_lossy();
    let num = match mcd.chars().nth_back(5).and_then(|c| c.to_digit(10)) {
        Some(1) => "".to_string(),
        Some(num) => format!(".{}", num - 1),
        None => "".to_string(),
    };

    let title = capitalize_first_letters(info.title.to_lowercase());
    let srm = title + " " + region + num.as_str() + ".srm";
    let mut srm_path = des.as_ref().to_path_buf();
    srm_path.push(srm);
    fs::copy(&mcd_path, &srm_path)?;
    println!(
        "Copied: {}\t->\t{}",
        mcd_path.as_ref().display(),
        srm_path.display()
    );
    Ok(())
}

fn convert<P: AsRef<Path>>(src: P, conn: &Connection, des: P) -> MemCardResult<()> {
    for mcd_path in find_mcds(src)? {
        mcd_to_srm(mcd_path.as_path(), conn, des.as_ref())?;
    }
    Ok(())
}

fn capitalize_first<S: AsRef<str>>(s: S) -> String {
    let mut c = s.as_ref().chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn capitalize_first_letters<S: AsRef<str>>(text: S) -> String {
    text.as_ref()
        .split_whitespace()
        .map(|word| capitalize_first(word))
        .collect::<Vec<String>>()
        .join(" ")
}
