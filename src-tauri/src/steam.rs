use crate::model::WorkshopItem;
use scraper::{Html, Selector};
use std::{collections::HashSet, fs, path::PathBuf};

const WORKSHOP_APP_ID: &str = "431960";

pub async fn search_workshop(query: &str, page: u32, library_roots: &[String]) -> anyhow::Result<Vec<WorkshopItem>> {
    let page = page.max(1);
    let url = format!(
        "https://steamcommunity.com/workshop/browse/?appid={WORKSHOP_APP_ID}&searchtext={}&browsesort=trend&section=readytouseitems&p={page}",
        urlencoding::encode(query)
    );
    let client = reqwest::Client::builder()
        .user_agent("OpenWallpaper/0.1 (+https://github.com/openwallpaper/openwallpaper)")
        .timeout(std::time::Duration::from_secs(18))
        .build()?;
    let response = client.get(&url).send().await?.error_for_status()?;
    let body = response.text().await?;
    let document = Html::parse_document(&body);
    let item_selector = Selector::parse(".workshopItem").unwrap();
    let link_selector = Selector::parse("a[href*='sharedfiles/filedetails']").unwrap();
    let title_selector = Selector::parse(".workshopItemTitle").unwrap();
    let image_selector = Selector::parse("img").unwrap();
    let installed_ids = installed_workshop_ids(library_roots);

    let mut items = Vec::new();
    for card in document.select(&item_selector) {
        let Some(link) = card.select(&link_selector).next() else { continue; };
        let href = link.value().attr("href").unwrap_or_default();
        let Some(id) = published_file_id(href) else { continue; };
        let title = card.select(&title_selector).next()
            .map(|element| element.text().collect::<String>().trim().to_string())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| format!("Workshop item {id}"));
        let preview_url = card.select(&image_selector).next().and_then(|image| {
            image.value().attr("src").or_else(|| image.value().attr("data-src")).map(str::to_string)
        });
        items.push(WorkshopItem {
            published_file_id: id.clone(),
            title,
            preview_url,
            item_url: format!("https://steamcommunity.com/sharedfiles/filedetails/?id={id}"),
            installed: installed_ids.contains(&id),
        });
    }
    items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()));
    items.dedup_by(|a, b| a.published_file_id == b.published_file_id);
    Ok(items)
}

pub async fn get_published_file_details(published_file_id: &str) -> anyhow::Result<serde_json::Value> {
    validate_published_file_id(published_file_id)?;
    let client = reqwest::Client::new();
    let url = "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";
    let params = [
        ("itemcount", "1".to_string()),
        ("publishedfileids[0]", published_file_id.to_string()),
    ];
    let response: serde_json::Value = client.post(url).form(&params).send().await?.error_for_status()?.json().await?;
    Ok(response)
}

pub fn open_workshop_item(published_file_id: &str) -> anyhow::Result<()> {
    validate_published_file_id(published_file_id)?;
    open::that(format!("steam://url/CommunityFilePage/{published_file_id}"))?;
    Ok(())
}

fn published_file_id(url: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key == "id" && validate_published_file_id(value).is_ok() { Some(value.to_string()) } else { None }
    })
}

pub fn validate_published_file_id(id: &str) -> anyhow::Result<()> {
    if id.len() < 5 || id.len() > 20 || !id.bytes().all(|b| b.is_ascii_digit()) {
        anyhow::bail!("Некорректный Steam PublishedFileID");
    }
    Ok(())
}

fn installed_workshop_ids(roots: &[String]) -> HashSet<String> {
    let mut ids = HashSet::new();
    for root in roots {
        let path = PathBuf::from(root);
        let text = path.to_string_lossy().to_lowercase();
        if !text.contains("431960") && !text.contains("workshop") { continue; }
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                        if validate_published_file_id(name).is_ok() { ids.insert(name.to_string()); }
                    }
                }
            }
        }
    }
    ids
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_non_numeric_workshop_ids() {
        assert!(validate_published_file_id("123456789").is_ok());
        assert!(validate_published_file_id("abc123").is_err());
        assert!(validate_published_file_id("1").is_err());
    }

    #[test]
    fn extracts_workshop_id_from_url() {
        assert_eq!(published_file_id("https://steamcommunity.com/sharedfiles/filedetails/?id=123456789"), Some("123456789".into()));
    }
}
