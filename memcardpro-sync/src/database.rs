use anyhow::{Result, anyhow};
use sqlite::Connection;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::FileNameInfo;

static REGION_MAP: std::sync::LazyLock<HashMap<&'static str, &'static str>> =
    std::sync::LazyLock::new(|| {
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
    });
static REGION_MAP_REV: std::sync::LazyLock<HashMap<&'static str, &'static str>> =
    std::sync::LazyLock::new(|| REGION_MAP.iter().map(|(k, v)| (*v, *k)).collect());

#[derive(Clone, Debug)]
pub struct GameInfo {
    pub code: Box<str>,
    pub title: Box<str>,
    pub lang: Box<str>,
}

#[derive(Clone, Debug)]
pub enum Game {
    Ps1(GameInfo),
    Ps1Mod(GameInfo, Arc<str>), // the second field is the patch code
}

impl Game {
    /// Creates a `Game` from a game code.
    /// If the game is a mod, it will return `Game::Ps1Mod`
    ///
    /// # Errors
    /// 1. If the game code does not exist in the database.
    /// 2. If query fails.
    pub fn new(code: &str, conn: &Connection) -> Result<Option<Self>> {
        let is_mod = conn
            .prepare("SELECT * FROM ps1_mods WHERE code = ?;")?
            .into_iter()
            .bind((1, code))?
            .next()
            .transpose()?
            .is_some();

        let info = GameInfo::new(code, conn)?;
        let patch_code = if is_mod {
            info.as_ref()
                .and_then(|info| info.get_patch_code(conn).ok().flatten())
        } else {
            None
        };
        let game = match (info, patch_code) {
            (Some(info), Some(patch_code)) => Some(Self::Ps1Mod(info, patch_code)),
            (Some(info), None) => Some(Self::Ps1(info)),
                _ => None,
        };
        Ok(game)
    }
    pub fn from_filename(srm: &FileNameInfo, conn: &Connection) -> Result<Option<Game>> {
        let patch = srm.patch.clone();
        let info = GameInfo::from_filename(&srm, conn)?;
        let game = match patch {
            Some(patch) => match info {
                Some(info) => Some(Game::Ps1Mod(info, patch.clone())),
                None => None,
            },
            None => info.map(Game::Ps1),
        };
        Ok(game)
    }
    pub fn get_region(&self) -> Result<Option<&'static str>> {
        self.get_info().get_region()
    }
    pub fn get_info(&self) -> &GameInfo {
        match self {
            Game::Ps1(info) | Game::Ps1Mod(info, _) => info,
        }
    }
}

impl GameInfo {
    /// loads game information from code
    ///
    /// # Errors
    /// 1. If the game code does not exist in the database.
    /// 2. If query fails.
    pub fn new<S: AsRef<str> + std::fmt::Debug>(
        code: S,
        conn: &Connection,
    ) -> Result<Option<Self>> {
        let query = "
            SELECT *
            FROM (SELECT * from ps1
            UNION ALL
            SELECT ps1_mods.code, ps1_mods.title, ps1.language FROM ps1_mods, ps1 ON ps1_mods.og_code = ps1.code)
            WHERE code = ?;";

        let info = conn
            .prepare(query)?
            .into_iter()
            .bind((1, code.as_ref()))?
            .map(|rrow| {
                rrow.map(|row| GameInfo {
                    code: row.read::<&str, _>("code").into(),
                    title: row.read::<&str, _>("title").into(),
                    lang: row.read::<&str, _>("language").into(),
                })
            })
            .nth(0)
            .transpose()?;
        Ok(info)
    }
    /// Gets the region of the game: (USA), (Europe), (Japan); or nothing for patched/homebrew games.
    pub fn get_region(&self) -> Result<Option<&'static str>> {
        let region_code = self
            .code
            .split_once('-')
            .ok_or(anyhow!(
                "{} does not following code naming convention",
                self.code
            ))?
            .0;
        Ok(REGION_MAP.get(region_code).map(|v| &**v))
    }
    /// Get patch code for modded games, or None for unmodded games.
    pub fn get_patch_code(&self, conn: &Connection) -> Result<Option<Arc<str>>> {
        let patch_code = conn
            .prepare("SELECT patch FROM ps1_mods WHERE code = ?;")?
            .into_iter()
            .bind((1, &*self.code))?
            .map(|rrow| rrow.map(|row| row.read::<&str, _>("patch").into()))
            .nth(0)
            .transpose()?;
        Ok(patch_code)
    }
    /// loads game information from srm name
    pub fn from_filename(srm: &FileNameInfo, conn: &Connection) -> Result<Option<GameInfo>> {
        let query = "SELECT code FROM ps1 WHERE title = ?;";
        let codes = conn
            .prepare(query)?
            .into_iter()
            .bind((1, &*srm.title))?
            .map(|rrow| rrow.map(|row| row.read::<&str, _>("code").into()))
            .collect::<Result<Vec<Arc<str>>, sqlite::Error>>()?;
        let code_prefix = REGION_MAP_REV
            .get(&*srm.region)
            .ok_or(anyhow!("region {} not found in database", srm.region))?;
        let code = codes
            .iter()
            .find(|code| code.split_once('-').map(|(prefix, _)| prefix) == Some(code_prefix));
        let info = match code {
            Some(code) => GameInfo::new(code, conn)?,
            None => None,
        };
        Ok(info)
    }
}

/// Gets the region of the game: (USA), (Europe), (Japan); or nothing for patched/homebrew games.
pub fn get_region<S: AsRef<str> + std::fmt::Debug>(code: S) -> Result<Option<&'static str>> {
    let region_code = code
        .as_ref()
        .split_once('-')
        .ok_or(anyhow!(
            "{code:?} does not following code naming convention"
        ))?
        .0;
    Ok(REGION_MAP.get(region_code).map(|v| &**v))
}
