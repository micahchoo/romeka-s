//! The bulk edit tests' shared ground: an import test site, people signed in with a role, and
//! the requests that preview, run and undo a bulk change as the site administrator.

#![allow(clippy::unwrap_used, clippy::expect_used, dead_code)]

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

pub use crate::import_site::{Site, site};
use omeka_db::ids::ItemSetId;
use omeka_db::repos::{ItemRepo, UserRepo};
use omeka_db::rows::{NewItem, NewResource, NewUser, NewWholeItem};
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub const PASSWORD: &str = "a long test password";

/// A signed-in person other than the site's administrator.
pub struct Person {
    pub addr: SocketAddr,
    pub cookie: String,
}

impl Person {
    pub async fn new(site: &Site, role: &str) -> Self {
        let email = format!("{role}@example.org");
        UserRepo::new(&site.pool)
            .create(&NewUser {
                email: email.clone(),
                name: role.to_string(),
                password_hash: Some(bcrypt::hash(PASSWORD, 4).unwrap()),
                role: role.to_string(),
                is_active: true,
                created: None,
            })
            .await
            .unwrap();
        let mut person = Self {
            addr: site.addr,
            cookie: String::new(),
        };
        let (status, head, body) = person
            .raw(
                "POST",
                "/api/login",
                Some(json!({ "email": email, "password": PASSWORD })),
            )
            .await;
        assert_eq!(status, 200, "{body}");
        let cookie = head
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("set-cookie")
                    .then(|| value.trim())
            })
            .expect("a session cookie");
        person.cookie = cookie.split(';').next().unwrap().to_string();
        person
    }

    pub async fn send(&self, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
        let (status, _, body) = self.raw(method, path, body).await;
        (status, serde_json::from_str(&body).unwrap_or(Value::Null))
    }

    pub async fn raw(
        &self,
        method: &str,
        path: &str,
        body: Option<Value>,
    ) -> (u16, String, String) {
        let body = body.map(|body| body.to_string()).unwrap_or_default();
        let mut request = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nOrigin: http://localhost\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        );
        if !self.cookie.is_empty() {
            request.push_str(&format!("Cookie: {}\r\n", self.cookie));
        }
        request.push_str("\r\n");
        request.push_str(&body);
        let mut stream = tokio::net::TcpStream::connect(self.addr).await.unwrap();
        stream.write_all(request.as_bytes()).await.unwrap();
        let mut raw = Vec::new();
        stream.read_to_end(&mut raw).await.unwrap();
        let raw = String::from_utf8_lossy(&raw).into_owned();
        let (head, body) = raw.split_once("\r\n\r\n").expect("a head");
        let body = if head
            .to_ascii_lowercase()
            .contains("transfer-encoding: chunked")
        {
            unchunk(body)
        } else {
            body.to_string()
        };
        (head[9..12].parse().unwrap(), head.to_string(), body)
    }
}

pub fn unchunk(mut rest: &str) -> String {
    let mut out = String::new();
    while let Some((size, after)) = rest.split_once("\r\n") {
        let size = usize::from_str_radix(size.trim(), 16).unwrap_or(0);
        if size == 0 {
            break;
        }
        out.push_str(&after[..size]);
        rest = &after[size + 2..];
    }
    out
}

/// The site administrator, as a [`Person`]'s answers read.
pub async fn admin(site: &Site, method: &str, path: &str, body: Option<Value>) -> (u16, Value) {
    let reply = site.send(method, path, body).await;
    (
        reply.status,
        serde_json::from_str(&reply.body).unwrap_or(Value::Null),
    )
}

pub fn title(text: &str) -> Value {
    json!([{ "type": "literal", "property_id": 1, "@value": text }])
}

pub async fn item_set(site: &Site, name: &str) -> u64 {
    let (status, set) = admin(
        site,
        "POST",
        "/api/item_sets",
        Some(json!({ "dcterms:title": title(name) })),
    )
    .await;
    assert_eq!(status, 200, "{set}");
    set["o:id"].as_u64().unwrap()
}

pub async fn item(site: &Site, name: &str, sets: &[u64]) -> u64 {
    let sets: Vec<Value> = sets.iter().map(|id| json!({ "o:id": id })).collect();
    let (status, item) = admin(
        site,
        "POST",
        "/api/items",
        Some(json!({ "dcterms:title": title(name), "o:item_set": sets, "o:is_public": true })),
    )
    .await;
    assert_eq!(status, 200, "{item}");
    item["o:id"].as_u64().unwrap()
}

pub async fn class(site: &Site, term: &str) -> u64 {
    let (status, classes) = admin(
        site,
        "GET",
        &format!("/api/resource_classes?term={term}"),
        None,
    )
    .await;
    assert_eq!(status, 200, "{classes}");
    classes[0]["o:id"].as_u64().unwrap()
}

/// What a bulk edit may change on an item, as `GET /api/items/{id}` shows it.
#[derive(Debug, PartialEq, Eq)]
pub struct Shown {
    pub public: bool,
    pub class: Option<u64>,
    pub sets: BTreeSet<u64>,
}

pub async fn shown(site: &Site, id: u64) -> Shown {
    let (status, item) = admin(site, "GET", &format!("/api/items/{id}"), None).await;
    assert_eq!(status, 200, "{item}");
    Shown {
        public: item["o:is_public"].as_bool().unwrap(),
        class: item["o:resource_class"]["o:id"].as_u64(),
        sets: item["o:item_set"]
            .as_array()
            .unwrap()
            .iter()
            .map(|set| set["o:id"].as_u64().unwrap())
            .collect(),
    }
}

pub async fn preview(site: &Site, query: Value, change: Value) -> Value {
    let (status, preview) = admin(
        site,
        "POST",
        "/api/romeka/bulk",
        Some(json!({ "query": query, "change": change })),
    )
    .await;
    assert_eq!(status, 201, "{preview}");
    preview
}

/// Wait until no job works on the run, and answer it.
pub async fn settled(site: &Site, id: u64) -> Value {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let (status, run) = admin(site, "GET", &format!("/api/romeka/bulk/{id}"), None).await;
        assert_eq!(status, 200, "{run}");
        let live = ["running", "stopping", "undoing", "undo_stopping"];
        if !live.contains(&run["status"].as_str().unwrap()) {
            return run;
        }
        assert!(
            Instant::now() < deadline,
            "run {id} stayed {}",
            run["status"]
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

pub async fn run(site: &Site, id: u64) -> Value {
    let (status, started) = admin(site, "POST", &format!("/api/romeka/bulk/{id}/run"), None).await;
    assert_eq!(status, 200, "{started}");
    settled(site, id).await
}

pub async fn undo(site: &Site, id: u64, force: bool) -> Value {
    let (status, started) = admin(
        site,
        "POST",
        &format!("/api/romeka/bulk/{id}/undo"),
        Some(json!({ "force": force })),
    )
    .await;
    assert_eq!(status, 200, "{started}");
    settled(site, id).await
}

/// Wait out the second the run wrote in: times are whole seconds.
pub async fn next_second() {
    tokio::time::sleep(Duration::from_millis(1100)).await;
}

/// Enough public items that the job is still at work when the test acts on it, written straight
/// to the store: the API would take too long to make them.
pub async fn postcards(site: &Site, sets: &[u64]) -> Vec<u64> {
    let mut items = Vec::new();
    for n in 0..1000 {
        let new = NewWholeItem {
            item: NewItem {
                resource: NewResource {
                    title: Some(format!("Postcard {n}")),
                    is_public: true,
                    ..NewResource::default()
                },
                primary_media_id: None,
            },
            item_sets: sets.iter().map(|id| ItemSetId(*id)).collect(),
            ..NewWholeItem::default()
        };
        items.push(
            ItemRepo::new(&site.pool)
                .create_whole(&new)
                .await
                .unwrap()
                .id()
                .0,
        );
    }
    items
}

pub async fn start(site: &Site, id: u64) {
    let (status, started) = admin(site, "POST", &format!("/api/romeka/bulk/{id}/run"), None).await;
    assert_eq!(status, 200, "{started}");
}
