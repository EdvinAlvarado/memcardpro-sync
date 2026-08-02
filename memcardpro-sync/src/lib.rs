mod database;
use crate::database::*;
use anyhow::{Result, anyhow};
use regex::Regex;
use sqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn capitalize_first<S: AsRef<str>>(s: S) -> String {
    let mut c = s.as_ref().chars();
    match c.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
    }
}

pub fn capitalize_first_letters<S: AsRef<str>>(text: S) -> String {
    text.as_ref()
        .split_whitespace()
        .map(|word| capitalize_first(word))
        .collect::<Vec<String>>()
        .join(" ")
}

static SRM_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(
        r"^(?P<title>.+?) (?P<region>\([^)]+\))(?: (?P<patch>\([^)]+\)))?_(?P<savenum>\d+)\.srm$",
    )
    .unwrap()
});

pub struct SaveFile {
    path: PathBuf,
    savenum: u32,
    game: database::Game,
}

pub struct FileNameInfo {
    pub title: Box<str>,
    pub region: Box<str>,
    pub patch: Option<Arc<str>>,
    pub savenum: u32,
}

impl SaveFile {
    fn from_memcardpro(path: PathBuf, conn: &Connection) -> Result<Option<Self>> {
        let code = path
            .parent()
            .ok_or(anyhow!("mcd path has no parent"))?
            .file_name()
            .ok_or(anyhow!("mcd path has no parent dir name"))?
            .to_string_lossy();
        let savenum =
            path.file_name()
                .ok_or(anyhow!("mcd path has no file name"))?
                .to_string_lossy()
                .chars()
                .nth_back(4)
                .and_then(|c| c.to_digit(10))
                .ok_or(anyhow!("mcd path does not follow naming convention"))? as u32;
        let game = database::Game::new(&code, conn)?;

        match game {
            Some(game) => Ok(Some(SaveFile {
                path,
                savenum,
                game,
            })),
            None => Ok(None),
        }
    }
    fn from_filename(path: PathBuf, conn: &Connection) -> Result<Option<Self>> {
        let filename = path.to_string_lossy();

        let caps = SRM_RE.captures(&filename).ok_or(anyhow!(
            "srm name does not follow naming convention: {}",
            path.display()
        ))?;

        let title = caps["title"].into();
        let region = caps["region"].into();
        let patch = caps.name("patch").map(|m| m.as_str().into());
        let savenum = caps["savenum"].parse()?;

        let filename_info = FileNameInfo {
            title,
            region,
            patch,
            savenum,
        };

        let game = database::Game::from_filename(filename_info, conn)?;
        Ok(match game {
            Some(game) => Some(SaveFile {
                path,
                savenum,
                game,
            }),
            None => None,
        })
    }
    /// Creates emulator-friendly save file name from memcardpro save file name.
    fn get_filename(&self) -> Result<Box<str>> {
        let info = self.game.get_info();
        let region = info.get_region()?;
        let title = capitalize_first_letters(info.title.to_lowercase());
        let patch = match &self.game {
            database::Game::Ps1(_) => None,
            database::Game::Ps1Mod(_, patch) => Some(patch.clone()),
        }
        .unwrap_or_default();

        let filename = format!(
            "{title} {region}{patch}_{savenum}.mcd",
            region = region.unwrap_or(""),
            savenum = self.savenum
        );
        Ok(filename.into())
    }
    /// get memcardpro save file name from emulator-friendly save file name.
    fn get_memcardpro_filename(&self) -> Result<Box<str>> {
        let code = self.game.get_info().code.as_ref();
        let savenum = self.savenum;

        let filename = format!("{code}-{savenum}.mcd");
        Ok(filename.into())
    }
    /// write the save file to the destination directory with the emulator-friendly name.
    fn write_to_saves(&self, save_path: &Path) -> Result<()> {
        let filename = self.get_filename()?;
        let mut des = save_path.to_path_buf();
        des.push(filename.as_ref());
        std::fs::copy(&self.path, &des)?;
        println!("Copied: {}\t->\t{}", self.path.display(), des.display());
        Ok(())
    }
    /// write the save file to the destination directory with the memcardpro name.
    fn write_to_memcardpro(&self, memcardpro_path: &Path, conn: &Connection) -> Result<()> {
        let filename = self.get_memcardpro_filename()?;
        let mut des = memcardpro_path.to_path_buf();

        des.push(self.game.get_info().code.as_ref());
        std::fs::create_dir_all(&des)?;

        des.push(filename.as_ref());
        std::fs::copy(&self.path, &des)?;
        println!("Copied: {}\t->\t{}", self.path.display(), des.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capitalize_first_test() {
        let text = "hello world";
        assert_eq!(capitalize_first_letters(text), "Hello World");
    }

    #[test]
    fn mcdfile_creation_test() {
        let path = PathBuf::from("../../tests/memcardpro/SCUS-94423/SCUS-94423-1.mcd");
        let conn = sqlite::open("../../tests/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_memcardpro(path, &conn).unwrap().unwrap();
        assert_eq!(savefile.game.get_info().code, "SCUS-94423".into());
        assert_eq!(savefile.savenum, 1);
    }
    #[test]
    fn mcdfile_translate_test() {
        let path = PathBuf::from("../../tests/memcardpro/SCUS-94423/SCUS-94423-1.mcd");
        let conn = sqlite::open("../../tests/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_memcardpro(path, &conn).unwrap().unwrap();
        let filename = savefile.get_filename().unwrap();
        assert_eq!(filename, "Ape Escape (USA)_1.mcd".into());
    }
    #[test]
    fn mcdfile_translate_patched_test() {
        let path = PathBuf::from("../../tests/memcardpro/TLWL-94221/TLWL-94221-1.mcd");
        let conn = sqlite::open("../../tests/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_memcardpro(path, &conn).unwrap().unwrap();
        assert_eq!(
            savefile.get_filename().unwrap(),
            "Final Fantasy Tactics (USA) (patched TLWotL v1.02)_1.mcd".into()
        );
    }
    #[test]
    fn savefile_translate_test() {
        let path = PathBuf::from("~/Documents/saves/psx/Ape Escape (USA)_1.mcd");
        let conn = sqlite::open("../../tests/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_filename(path, &conn).unwrap().unwrap();
        let filename = savefile.get_memcardpro_filename().unwrap();
        assert_eq!(filename, "SCUS-94423-1.mcd".into());
    }
    #[test]
    fn savefile_translate_patched_test() {
        let path = PathBuf::from(
            "~/Documents/saves/psx/Final Fantasy Tactics (USA) (patched TLWotL v1.02)_1.mcd",
        );
        let conn = sqlite::open("../../tests/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_filename(path, &conn).unwrap().unwrap();
        assert_eq!(
            savefile.get_memcardpro_filename().unwrap(),
            "TLWL-94221-1.mcd".into()
        );
    }
}
