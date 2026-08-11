mod database;
use anyhow::{anyhow, Result};
use regex::Regex;
use sqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn capitalize_first<S: AsRef<str>>(s: S) -> String {
    let mut c = s.as_ref().chars();
    c.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + c.as_str()
    })
}

pub fn capitalize_first_letters<S: AsRef<str>>(text: S) -> String {
    text.as_ref()
        .split_whitespace()
        .map(capitalize_first)
        .collect::<Vec<String>>()
        .join(" ")
}

#[allow(clippy::unwrap_used)]
static SRM_RE: std::sync::LazyLock<Regex> = std::sync::LazyLock::new(|| {
    Regex::new(r"^(?P<title>.+?)\s+(?P<region>\([^)]+\))(?:\s+\((?P<patch>[^)]+)\))?_(?P<savenum>\d+)\.mcd$"
    )
    .unwrap()
});

pub struct SaveFile {
    pub path: PathBuf,
    pub savenum: u32,
    pub game: database::GameInfo,
}

#[derive(Debug)]
pub struct FileNameInfo {
    pub title: Box<str>,
    pub region: Box<str>,
    pub patch: Option<Arc<str>>,
    pub savenum: u32,
}

impl SaveFile {
    /// Creates a `SaveFile` from a memcardpro save file path.
    ///
    /// # Errors
    /// 1. If the path does not have a parent directory.
    /// 2. If the parent directory does not have a name.
    /// 3. If the path does not have a file name.
    /// 4. If the file name does not follow the naming convention.
    pub fn from_memcardpro(path: PathBuf, conn: &Connection) -> Result<Self> {
        let code = path
            .parent()
            .ok_or_else(|| anyhow!("mcd path has no parent"))?
            .file_name()
            .ok_or_else(|| anyhow!("mcd path has no parent dir name"))?
            .to_string_lossy();
        let savenum = path
            .file_name()
            .ok_or_else(|| anyhow!("mcd path has no file name"))?
            .to_string_lossy()
            .chars()
            .nth_back(4)
            .and_then(|c| c.to_digit(10))
            .ok_or_else(|| anyhow!("mcd path does not follow naming convention"))?;
        let game = database::GameInfo::new(&code, conn)?.ok_or_else(|| anyhow!("game code not found in database: {code}. If this code is for a PS1 mod, please submit issue ticket on github."))?;

        Ok(Self {
            path,
            savenum,
            game,
        })
    }
    /// Creates a `SaveFile` from an emulator-friendly save file path.
    ///
    /// # Errors
    /// 1. If the file name does not follow the naming convention.
    pub fn from_filename(path: PathBuf, conn: &Connection) -> Result<Self> {
        let filename = path
            .file_name()
            .ok_or_else(|| anyhow!("path has no file name"))?
            .to_string_lossy();

        let caps = SRM_RE.captures(&filename).ok_or_else(|| {
            anyhow!(
                "srm name does not follow naming convention: {}",
                path.display()
            )
        })?;

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

        let game = database::GameInfo::from_filename(&filename_info, conn)?
            .ok_or_else(|| anyhow!("game not found in database for filename: {filename_info:?}"))?;
        Ok(Self {
            path,
            savenum,
            game,
        })
    }
    /// Creates emulator-friendly save file name from memcardpro save file name.
    pub fn get_filename(&self) -> Box<str> {
        let title = self.game.get_title();

        let filename = format!("{title}_{savenum}.mcd", savenum = self.savenum);
        filename.into()
    }
    /// get memcardpro save file name from emulator-friendly save file name.
    pub fn get_memcardpro_filename(&self) -> Box<str> {
        let code = self.game.code.as_ref();
        let savenum = self.savenum;

        format!("{code}-{savenum}.mcd").into_boxed_str()
    }
    /// write the save file to the destination directory with the emulator-friendly name.
    ///
    /// # Errors
    /// 1. Cannot find region for `Game`.
    /// 2. If the destination directory does not exist and cannot be created.
    /// 3. If the save file cannot be copied to the destination directory.
    pub fn write_to_saves(&self, save_path: &Path) -> Result<()> {
        let filename = self.get_filename();

        let mut new_save_path = save_path.to_path_buf();
        new_save_path.push(filename.as_ref());
        std::fs::create_dir_all(save_path)?;
        std::fs::copy(&self.path, &new_save_path)?;
        println!(
            "Copied: {}\t->\t{}",
            self.path.display(),
            new_save_path.display()
        );
        Ok(())
    }
    /// write the save file to the destination directory with the memcardpro name.
    ///
    /// # Errors
    /// 1. If the destination directory does not exist and cannot be created.
    /// 2. If the save file cannot be copied to the destination directory.
    pub fn write_to_memcardpro(&self, memcardpro_path: &Path) -> Result<()> {
        let filename = self.get_memcardpro_filename();
        let mut des = memcardpro_path.to_path_buf();

        des.push(self.game.code.as_ref());
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
        let path = PathBuf::from("tests/memcardpro/SCUS-94423/SCUS-94423-1.mcd");
        let conn = sqlite::open("data/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_memcardpro(path, &conn).unwrap();
        assert_eq!(savefile.game.code, "SCUS-94423".into());
        assert_eq!(savefile.savenum, 1);
    }
    #[test]
    fn mcdfile_translate_test() {
        let path = PathBuf::from("tests/memcardpro/SCUS-94423/SCUS-94423-1.mcd");
        let conn = sqlite::open("data/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_memcardpro(path, &conn).unwrap();
        let filename = savefile.get_filename();
        println!("filename: {}", filename);
        assert_eq!(filename, "Ape Escape (USA)_1.mcd".into());
    }
    #[test]
    fn mcdfile_translate_patched_test() {
        let path = PathBuf::from("tests/memcardpro/TLWL-94221/TLWL-94221-1.mcd");
        let conn = sqlite::open("data/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_memcardpro(path, &conn).unwrap();
        println!("filename: {}", savefile.get_filename());
        assert_eq!(
            savefile.get_filename(),
            "Final Fantasy Tactics (USA) (patched TLWotL v1.02)_1.mcd".into()
        );
    }
    #[test]
    fn savefile_translate_test() {
        let path = PathBuf::from("~/Documents/saves/psx/Ape Escape (USA)_1.mcd");
        let conn = sqlite::open("data/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_filename(path, &conn).unwrap().unwrap();
        let filename = savefile.get_memcardpro_filename();
        println!("filename: {}", filename);
        assert_eq!(filename, "SCUS-94423-1.mcd".into());
    }
    #[test]
    fn savefile_translate_patched_test() {
        let path = PathBuf::from(
            "~/Documents/saves/psx/Final Fantasy Tactics (USA) (patched TLWotL v1.02)_1.mcd",
        );
        let conn = sqlite::open("data/ps1_games.sqlite3").unwrap();
        let savefile = SaveFile::from_filename(path, &conn).unwrap().unwrap();
        assert_eq!(
            savefile.get_memcardpro_filename(),
            "TLWL-94221-1.mcd".into()
        );
    }
}
