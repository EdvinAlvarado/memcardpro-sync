use anyhow::{Result, anyhow};
use lazy_static::lazy_static;
use sqlite::Connection;
use std::collections::HashMap;
use std::path::Path;

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
        m.insert("TLWL", ""); // custom code for FFT TlWotL patch
        m
    };
}

#[derive(Clone, Debug)]
pub struct GameInfo {
    pub code: Box<str>,
    pub title: Box<str>,
    pub lang: Box<str>,
}

impl GameInfo {
    /// Gets the region of the game: (USA), (Europe), (Japan); or nothing for patched/homebrew games.
    pub fn get_region(&self) -> Result<Box<str>> {
        let region_code = self
            .code
            .split_once('-')
            .ok_or(anyhow!(
                "{} does not following code naming convention",
                self.code
            ))?
            .0;
        let region = REGION_MAP.get(region_code).ok_or(anyhow!("could not find region for code. If modded game then program needs to be updated with it's custom code"))?;
        Ok((*region).into())
    }
    /// loads game information from code
    pub fn new<S: AsRef<str>>(code: S, conn: &Connection) -> Result<Option<GameInfo>> {
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
    /// loads game information from path
    pub fn from_path<P: AsRef<Path>>(mcd_path: P, conn: &Connection) -> Result<Option<GameInfo>> {
        let code = mcd_path
            .as_ref()
            .parent()
            .ok_or(anyhow!("mcd file without parent dir"))?
            .file_name()
            .ok_or(anyhow!("is this the correct dir?"))?
            .to_string_lossy();
        let mcd = mcd_path.as_ref().file_name().unwrap().to_string_lossy();

        GameInfo::new(&code, conn)
    }
}
