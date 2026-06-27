use super::*;

pub(crate) async fn fetch_token_inventory_snapshot(
    positions_base_url: Option<&str>,
    positions_page_size: i64,
    positions_max_pages: i64,
    user: &str,
    http: &Client,
) -> Result<Option<TokenInventorySnapshot>> {
    let Some(base_url) = positions_base_url else {
        return Ok(None);
    };
    let base_url = base_url.trim();
    if base_url.is_empty() {
        return Ok(None);
    }

    let limit = positions_page_size.max(1);
    let max_pages = positions_max_pages.max(1);
    let url = format!("{}/positions", base_url.trim_end_matches('/'));
    let limit_str = limit.to_string();
    let mut snapshot = TokenInventorySnapshot::default();

    for page in 0..max_pages {
        let offset = page * limit;
        let offset_str = offset.to_string();
        let rows = http
            .get(url.clone())
            .query(&[
                ("user", user),
                ("sizeThreshold", "0"),
                ("limit", limit_str.as_str()),
                ("offset", offset_str.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<Vec<DataApiInventoryPosition>>()
            .await?;

        if rows.is_empty() {
            break;
        }

        for row in &rows {
            let qty = parse_json_f64(row.size.as_ref())
                .or_else(|| parse_json_f64(row.balance.as_ref()))
                .unwrap_or_default();
            snapshot.add_position_row(
                [
                    row.asset.as_deref(),
                    row.token_id.as_deref(),
                    row.clob_token_id.as_deref(),
                ]
                .into_iter()
                .flatten(),
                qty,
            );
        }

        if rows.len() < limit as usize {
            break;
        }
    }

    Ok(Some(snapshot))
}
