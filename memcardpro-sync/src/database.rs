use anyhow::{anyhow, Result};
use sqlite::Connection;
use std::collections::HashMap;
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
static REGION_MAP_REV: std::sync::LazyLock<HashMap<&'static str, Vec<&'static str>>> =
    std::sync::LazyLock::new(|| {
        let mut m: HashMap<&str, Vec<&str>> = HashMap::new();
        for (k, v) in REGION_MAP.iter() {
            m.entry(*v).or_default().push(*k);
        }
        m
    });

static QUERY: std::sync::LazyLock<&'static str> = std::sync::LazyLock::new(|| {
    "SELECT code, title, language, NULL AS patch
    FROM ps1

    UNION ALL

    SELECT m.code, p.title, m.language, m.patch
    FROM ps1_mods m
    LEFT JOIN ps1 p ON m.og_code = p.code;"
});
static QUERY_FROM_FILENAME: std::sync::LazyLock<&'static str> = std::sync::LazyLock::new(|| {
    "SELECT code, title, language, NULL AS patch
    FROM ps1
    WHERE title = ?

    UNION ALL

    SELECT m.code, p.title, m.language, m.patch
    FROM ps1_mods m
    LEFT JOIN ps1 p ON m.og_code = p.code
    WHERE p.title = ?;"
});

#[derive(Clone, Debug)]
pub struct GameInfo {
    pub code: Box<str>,
    title: Box<str>,
    pub lang: Box<str>,
    patch: Option<Arc<str>>,
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
        let info = conn
            .prepare(*QUERY)?
            .into_iter()
            .bind((1, code.as_ref()))?
            .map(|rrow| {
                rrow.map(|row| GameInfo {
                    code: row.read::<&str, _>("code").into(),
                    title: row.read::<&str, _>("title").into(),
                    lang: row.read::<&str, _>("language").into(),
                    patch: row.read::<Option<&str>, _>("patch").map(|s| s.into()),
                })
            })
            .nth(0)
            .transpose()?;
        Ok(info)
    }
    /// Contruct title
    pub fn get_title(&self) -> Box<str> {
        let region = self.get_region().unwrap_or("");
        let title = crate::capitalize_first_letters(self.title.to_lowercase());

        let title = if let Some(patch) = &self.patch {
            format!("{} {} ({})", self.title, region, patch.to_uppercase())
        } else {
            format!("{} {}", self.title, region)
        };
        title.into()
    }
    /// Gets the region of the game: (USA), (Europe), (Japan); or nothing for patched/homebrew games.
    pub fn get_region(&self) -> Option<&'static str> {
        let region_code = self.code.split_once('-').map(|(prefix, _)| prefix)?;
        REGION_MAP.get(region_code).map(|v| &**v)
    }
    /// loads game information from srm name
    pub fn from_filename(srm: &FileNameInfo, conn: &Connection) -> Result<Option<GameInfo>> {
        eprintln!("TEMP TEST looking up game info for srm: {:?}", srm);
        let title = &*srm.title.to_uppercase();
        println!("TEMP TEST looking up game info for title: {title}");
        let games = conn
            .prepare(*QUERY_FROM_FILENAME)?
            .into_iter()
            .bind((1, title))?
            .map(|rrow| {
                rrow.map(|row| GameInfo {
                    code: row.read::<&str, _>("code").into(),
                    title: row.read::<&str, _>("title").into(),
                    lang: row.read::<&str, _>("language").into(),
                    patch: row.read::<Option<&str>, _>("patch").map(|s| s.into()),
                })
            })
            .collect::<Result<Vec<GameInfo>, sqlite::Error>>()?;
        println!("TEMP TEST found codes: {:?}", games);

        let code_prefixes = REGION_MAP_REV
            .get(&*srm.region)
            .ok_or(anyhow!("region {} not found in database", srm.region))?;
        println!(
            "TEMP TEST looking for code with prefix: {:?}",
            code_prefixes
        );

        let game = games.into_iter().find(|game| {
            let code_prefix = game.code.split_once('-').map(|(prefix, _)| prefix);
            match code_prefix {
                Some(prefix) => code_prefixes.contains(&prefix),
                None => false,
            }
        });

        println!("TEMP TEST found code: {:?}", game);

        Ok(game)
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
