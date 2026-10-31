//! Looks games up on PCGamingWiki on a background thread, at most one
//! request per second, and stores results for 30 days.
//! Feature doc: `docs/features/13-game-info.md`.

use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use huedra_core::gameinfo::{self, Cache, GameInfo, WikiHdr};
use huedra_core::library::GameId;
use huedra_win::http;

/// Minimum time between two requests.
const PACE: Duration = Duration::from_secs(1);

/// A game to look up.
pub struct Lookup {
    pub id: GameId,
    pub name: String,
}

/// Seconds since 1970, for cache ages.
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Starts the worker. `deliver` receives each new result; it runs on the
/// worker thread and must hand the result to the window itself.
pub fn spawn(items: Vec<Lookup>, cache_path: PathBuf, deliver: impl Fn(GameInfo) + Send + 'static) {
    let _ = thread::Builder::new()
        .name("huedra-lookups".into())
        .spawn(move || {
            let mut cache = Cache::load(&cache_path);
            let mut pacer = Pacer(None);
            for item in items {
                let now = now_secs();
                if cache.get(&item.id).is_some_and(|info| !info.is_stale(now)) {
                    continue;
                }
                let Some(info) = lookup(&item, now, &mut pacer) else {
                    // Network trouble: try again next time, keep going quietly.
                    continue;
                };
                cache.put(info.clone());
                let _ = cache.save(&cache_path);
                deliver(info);
            }
        });
}

struct Pacer(Option<Instant>);

impl Pacer {
    fn wait(&mut self) {
        if let Some(last) = self.0 {
            let since = last.elapsed();
            if since < PACE {
                thread::sleep(PACE - since);
            }
        }
        self.0 = Some(Instant::now());
    }
}

fn user_agent() -> String {
    format!(
        "Huedra/{} (Windows HDR switching; looks up HDR support per game)",
        huedra_core::VERSION
    )
}

fn get(path: &str, pacer: &mut Pacer) -> Option<http::Response> {
    pacer.wait();
    http::get(
        gameinfo::HOST,
        path,
        &user_agent(),
        gameinfo::MAX_RESPONSE_BYTES,
    )
    .ok()
}

/// One game: find its page (by Steam id, otherwise by title search), then
/// read the page's HDR fields. `None` means the network failed.
fn lookup(item: &Lookup, now: u64, pacer: &mut Pacer) -> Option<GameInfo> {
    let mut page = None;
    if let Some(app_id) = item.id.steam_app_id() {
        let response = get(&gameinfo::appid_path(app_id), pacer)?;
        if response.status == 200 {
            page = gameinfo::page_from_url(&response.final_url);
        }
    }
    if page.is_none() {
        let response = get(&gameinfo::search_path(&item.name), pacer)?;
        if response.status == 200 {
            page = gameinfo::search_result(&String::from_utf8_lossy(&response.body));
        }
    }
    let not_found = GameInfo {
        id: item.id.clone(),
        page: None,
        hdr: WikiHdr::Unknown,
        hdr_notes: String::new(),
        ray_tracing: String::new(),
        fetched: now,
    };
    let Some(page) = page else {
        return Some(not_found);
    };
    let response = get(&gameinfo::wikitext_path(&page), pacer)?;
    if response.status != 200 {
        return Some(not_found);
    }
    let body = String::from_utf8_lossy(&response.body);
    Some(gameinfo::parse_wikitext(&body, item.id.clone(), page, now).unwrap_or(not_found))
}
