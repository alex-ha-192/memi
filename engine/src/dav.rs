//! The parts of WebDAV that don't need a network: reading a server's PROPFIND answer, checking
//! the folder address, and naming files. Shared by the native client (webdav.rs) and the browser's.

/// What Memi asks the server about each file.
pub const PROPFIND_BODY: &str = r#"<?xml version="1.0" encoding="utf-8"?><d:propfind xmlns:d="DAV:"><d:prop><d:getetag/><d:getlastmodified/><d:resourcetype/></d:prop></d:propfind>"#;

/// One entry from a PROPFIND answer.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    pub etag: Option<String>,
    /// Seconds since 1970, from getlastmodified.
    pub modified: i64,
    pub folder: bool,
}

/// "alex on cloud.example.com"
pub fn account_label(user: &str, url: &str) -> String {
    let host = url.split("://").nth(1).unwrap_or(url).split('/').next().unwrap_or(url);
    format!("{user} on {host}")
}

/// Checks the address and makes it end in "/". Only https (or http to this machine or the local network).
pub fn normalise_url(url: &str) -> anyhow::Result<String> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    let host = lower.split("://").nth(1).unwrap_or("").split(['/', ':']).next().unwrap_or("");
    let local = host == "localhost" || host.starts_with("127.") || host.starts_with("192.168.") || host.starts_with("10.") || host.ends_with(".local");
    if !(lower.starts_with("https://") || (lower.starts_with("http://") && local)) {
        anyhow::bail!("The address needs to start with https:// (http:// is only allowed on your own network)");
    }
    if host.is_empty() { anyhow::bail!("That address has no server name"); }
    Ok(if url.ends_with('/') { url.to_string() } else { format!("{url}/") })
}

pub fn percent_encode(name: &str) -> String {
    name.bytes().map(|b| match b {
        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
        _ => format!("%{b:02X}"),
    }).collect()
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) { out.push(v); i += 3; continue; }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Reads a PROPFIND multistatus answer. Tolerant of any namespace prefix (d:, D:, none).
pub fn parse_propfind(xml: &str) -> Vec<Entry> {
    let mut out = vec![];
    let mut cur: Option<Entry> = None;
    let mut field: Option<&'static str> = None;
    let mut text = String::new();
    let mut rest = xml;
    while let Some(lt) = rest.find('<') {
        let before = &rest[..lt];
        if field.is_some() { text.push_str(before); }
        let Some(gt) = rest[lt..].find('>') else { break };
        let tag = &rest[lt + 1..lt + gt];
        rest = &rest[lt + gt + 1..];
        if tag.starts_with('?') || tag.starts_with('!') { continue; }
        let closing = tag.starts_with('/');
        let self_closing = tag.ends_with('/');
        let name = tag.trim_start_matches('/').trim_end_matches('/').split_whitespace().next().unwrap_or("");
        let local = name.rsplit(':').next().unwrap_or(name).to_ascii_lowercase();
        match (local.as_str(), closing) {
            ("response", false) => cur = Some(Entry::default()),
            ("response", true) => if let Some(e) = cur.take() { out.push(e) },
            ("collection", false) => if let Some(e) = cur.as_mut() { e.folder = true },
            ("href" | "getetag" | "getlastmodified", false) if !self_closing => {
                field = Some(match local.as_str() { "href" => "href", "getetag" => "etag", _ => "modified" });
                text.clear();
            }
            ("href" | "getetag" | "getlastmodified", true) => {
                if let (Some(f), Some(e)) = (field.take(), cur.as_mut()) {
                    let t = unescape(text.trim());
                    match f {
                        "href" => e.name = percent_decode(t.trim_end_matches('/').rsplit('/').next().unwrap_or("")),
                        "etag" => e.etag = Some(t.trim_matches('"').trim_start_matches("W/").trim_matches('"').to_string()).filter(|s| !s.is_empty()),
                        _ => e.modified = http_date(&t).unwrap_or(0),
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"").replace("&apos;", "'").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

/// "Sun, 06 Nov 1994 08:49:37 GMT" → seconds since 1970.
fn http_date(s: &str) -> Option<i64> {
    let ts = jiff::fmt::rfc2822::parse(s).ok()?;
    Some(ts.timestamp().as_second())
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_nextcloud_answer() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:s="http://sabredav.org/ns" xmlns:oc="http://owncloud.org/ns">
 <d:response><d:href>/remote.php/dav/files/alex/Memi/</d:href><d:propstat><d:prop>
  <d:getlastmodified>Sat, 21 Nov 2026 09:00:00 GMT</d:getlastmodified><d:resourcetype><d:collection/></d:resourcetype><d:getetag>&quot;abc&quot;</d:getetag>
 </d:prop><d:status>HTTP/1.1 200 OK</d:status></d:propstat></d:response>
 <d:response><d:href>/remote.php/dav/files/alex/Memi/memi-backup.bin</d:href><d:propstat><d:prop>
  <d:getlastmodified>Sat, 21 Nov 2026 10:00:00 GMT</d:getlastmodified><d:resourcetype/><d:getetag>&quot;5f1e&quot;</d:getetag>
 </d:prop></d:propstat></d:response>
 <D:response xmlns:D="DAV:"><D:href>/Memi/memi-join%2Dx_y.bin</D:href><D:propstat><D:prop>
  <D:getlastmodified>Sat, 21 Nov 2026 11:00:00 GMT</D:getlastmodified><D:getetag>W/"77"</D:getetag><D:resourcetype></D:resourcetype>
 </D:prop></D:propstat></D:response>
</d:multistatus>"#;
        let e = parse_propfind(xml);
        assert_eq!(e.len(), 3);
        assert!(e[0].folder);
        assert_eq!((e[1].name.as_str(), e[1].etag.as_deref(), e[1].folder), ("memi-backup.bin", Some("5f1e"), false));
        assert_eq!(e[1].modified, 1_795_255_200); // 21 Nov 2026, 10:00 UTC
        assert_eq!((e[2].name.as_str(), e[2].etag.as_deref()), ("memi-join-x_y.bin", Some("77")));
        assert!(e[2].modified > e[1].modified);
    }

    #[test]
    fn addresses() {
        assert_eq!(normalise_url(" https://cloud.example.com/dav/Memi ").unwrap(), "https://cloud.example.com/dav/Memi/");
        assert!(normalise_url("http://cloud.example.com/dav").is_err());
        assert!(normalise_url("http://192.168.1.5:8080/dav").is_ok());
        assert!(normalise_url("cloud.example.com").is_err());
        assert_eq!(account_label("alex", "https://cloud.example.com/remote.php/dav/files/alex/Memi/"), "alex on cloud.example.com");
        assert_eq!(percent_encode("memi-join-a_b.bin"), "memi-join-a_b.bin");
        assert_eq!(percent_decode("a%20b%2D"), "a b-");
    }
}
