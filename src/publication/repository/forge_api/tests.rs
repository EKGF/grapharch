//! Forge clients against a loopback mock API; no network.
use {
    super::{ForgeCache, Visibility, client::fetch},
    crate::publication::repository::forge::ForgeRepository,
    std::{
        collections::HashMap,
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    },
    tokio::io::{AsyncReadExt, AsyncWriteExt},
};

/// Serve `routes` (path and query → JSON) and count requests.
async fn serve(
    routes: HashMap<&'static str, &'static str>,
) -> (String, Arc<AtomicUsize>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let served = count.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut stream, _)) = listener.accept().await else {
                return
            };
            served.fetch_add(1, Ordering::SeqCst);
            let mut buffer = vec![0; 8192];
            let read = stream.read(&mut buffer).await.unwrap_or(0);
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            let target =
                request.split_whitespace().nth(1).unwrap_or("").to_owned();
            let (status, body) = match routes.get(target.as_str()) {
                Some(body) => ("200 OK", *body),
                None => ("404 Not Found", "{}"),
            };
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: \
                 application/json\r\ncontent-length: {}\r\nconnection: \
                 close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.ok();
        }
    });
    (
        format!("http://127.0.0.1:{}/api/", address.port()),
        count,
    )
}

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";

#[tokio::test]
async fn github_metadata_and_latest_release_are_read_and_cached() {
    let (base, count) = serve(HashMap::from([
        ("/api/repos/o/r", r#"{"full_name":"o/r","description":"A tool","homepage":"https://o.dev",
          "topics":["rdf","docs"],"license":{"spdx_id":"MIT"},"visibility":"public",
          "archived":false,"stargazers_count":7,"has_discussions":true}"#),
        ("/api/repos/o/r/releases/latest", r#"{"name":"One","tag_name":"v1.0.0",
          "published_at":"2026-01-02T03:04:05Z",
          "html_url":"https://github.com/o/r/releases/tag/v1.0.0"}"#),
    ]))
    .await;
    let cache_dir = tempfile::tempdir().unwrap();
    let cache = ForgeCache::new(cache_dir.path().to_owned());
    let repository = ForgeRepository::from_remote("https://github.com/o/r")
        .unwrap()
        .unwrap();
    let environment =
        |name: &str| (name == "GITHUB_API_URL").then(|| base.clone());
    let metadata = fetch(&repository, COMMIT, &cache, &environment)
        .await
        .unwrap();
    assert_eq!(metadata.description.as_deref(), Some("A tool"));
    assert_eq!(metadata.topics, ["rdf", "docs"]);
    assert_eq!(metadata.license().unwrap().as_str(), "MIT");
    assert_eq!(metadata.visibility, Some(Visibility::Public));
    assert!(metadata.discussions);
    let release = metadata.latest_release.clone().unwrap();
    assert_eq!(release.tag, "v1.0.0");
    assert_eq!(count.load(Ordering::SeqCst), 2);
    // The same commit is recorded input: no further requests.
    let again = fetch(&repository, COMMIT, &cache, &environment)
        .await
        .unwrap();
    assert_eq!(again, metadata);
    assert_eq!(count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn gitlab_maps_license_keys_and_release_links() {
    let (base, _) = serve(HashMap::from([
        ("/api/projects/g%2Fsub%2Fp?license=true", r#"{"path_with_namespace":"g/sub/p","description":"Lab",
          "topics":[],"license":{"key":"apache-2.0"},"visibility":"internal",
          "archived":true,"star_count":1}"#),
        ("/api/projects/g%2Fsub%2Fp/releases?per_page=1&order_by=released_at&sort=desc",
         r#"[{"name":"R2","tag_name":"2.0.0","released_at":"2026-02-01T00:00:00Z"}]"#),
    ]))
    .await;
    let repository = ForgeRepository::from_remote("git@gitlab.com:g/sub/p.git")
        .unwrap()
        .unwrap();
    let environment =
        |name: &str| (name == "CI_API_V4_URL").then(|| base.clone());
    let metadata = fetch(
        &repository,
        COMMIT,
        &ForgeCache::disabled(),
        &environment,
    )
    .await
    .unwrap();
    assert_eq!(metadata.license().unwrap().as_str(), "Apache-2.0");
    assert_eq!(metadata.visibility, Some(Visibility::Internal));
    assert!(metadata.archived);
    assert_eq!(
        metadata.latest_release.unwrap().notes.unwrap().as_str(),
        "https://gitlab.com/g/sub/p/-/releases/2.0.0"
    );
}

#[tokio::test]
async fn an_unreachable_forge_is_an_error_the_discoverer_can_degrade() {
    let repository = ForgeRepository::from_remote("https://github.com/o/r")
        .unwrap()
        .unwrap();
    let environment = |name: &str| {
        (name == "GITHUB_API_URL").then(|| "http://127.0.0.1:9/".to_owned())
    };
    assert!(
        fetch(
            &repository,
            COMMIT,
            &ForgeCache::disabled(),
            &environment
        )
        .await
        .is_err()
    );
}
