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
    "SELECT code, title, language, NULL AS patch, code AS region_code
    FROM ps1
    WHERE code = ?

    UNION ALL

    SELECT m.code, p.title, m.language, m.patch, m.og_code AS region_code
    FROM ps1_mods m
    LEFT JOIN ps1 p ON m.og_code = p.code
    WHERE m.code = ?;"
});
static QUERY_FROM_FILENAME: std::sync::LazyLock<&'static str> = std::sync::LazyLock::new(|| {
    "SELECT code, title, language, NULL AS patch, code AS region_code
    FROM ps1
    WHERE title = ?

    UNION ALL

    SELECT m.code, p.title, m.language, m.patch, m.og_code AS region_code
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
    region_code: Box<str>,
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
            .bind((2, code.as_ref()))?
            .map(|rrow| {
                rrow.map(|row| Self {
                    code: row.read::<&str, _>("code").into(),
                    title: row.read::<&str, _>("title").into(),
                    lang: row.read::<&str, _>("language").into(),
                    patch: row.read::<Option<&str>, _>("patch").map(Into::into),
                    region_code: row.read::<&str, _>("region_code").into(),
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
        let title = match (self.patch.as_deref(), region) {
            (Some(patch), "") => format!("{title} ({patch})"),
            (Some(patch), region) => format!("{title} {region} ({patch})"),
            (None, "") => title,
            (None, region) => format!("{title} {region}"),
        };
        title.into()
    }
    /// Gets the region of the game: (USA), (Europe), (Japan); or nothing for patched/homebrew games.
    pub fn get_region(&self) -> Option<&'static str> {
        let region_code = self.region_code.split_once('-').map(|(prefix, _)| prefix)?;
        REGION_MAP.get(region_code).map(|v| &**v)
    }
    /// loads game information from srm name
    pub fn from_filename(srm: &FileNameInfo, conn: &Connection) -> Result<Option<Self>> {
        let title = &*srm.title.to_uppercase();
        let games = conn
            .prepare(*QUERY_FROM_FILENAME)?
            .into_iter()
            .bind((1, title))?
            .bind((2, title))?
            .map(|rrow| {
                rrow.map(|row| Self {
                    code: row.read::<&str, _>("code").into(),
                    title: row.read::<&str, _>("title").into(),
                    lang: row.read::<&str, _>("language").into(),
                    patch: row.read::<Option<&str>, _>("patch").map(Into::into),
                    region_code: row.read::<&str, _>("region_code").into(),
                })
            })
            .collect::<Result<Vec<Self>, sqlite::Error>>()?;

        // If the srm has a patch, try to find a game with the same patch first.
        if let Some(patch) = srm.patch.as_deref() {
            let patched = games.iter().find(|game| {
                game.patch
                    .as_deref()
                    .is_some_and(|p| p.eq_ignore_ascii_case(patch))
            });
            if let Some(game) = patched {
                return Ok(Some(game.clone()));
            }
        }

        // If no game with the same patch was found, try to find a game with the same region.
        let code_prefixes = REGION_MAP_REV
            .get(&*srm.region)
            .ok_or_else(|| anyhow!("region {} not found in database", srm.region))?;

        let game = games.into_iter().find(|game| {
            game.code
                .split_once('-')
                .map(|(prefix, _)| prefix)
                .is_some_and(|prefix| code_prefixes.contains(&prefix))
        });

        Ok(game)
    }
}

/// Gets the region of the game: (USA), (Europe), (Japan); or nothing for patched/homebrew games.
pub fn get_region<S: AsRef<str> + std::fmt::Debug>(code: S) -> Result<Option<&'static str>> {
    let region_code = code
        .as_ref()
        .split_once('-')
        .ok_or_else(|| anyhow!("{code:?} does not following code naming convention"))?
        .0;
    Ok(REGION_MAP.get(region_code).map(|v| &**v))
}
