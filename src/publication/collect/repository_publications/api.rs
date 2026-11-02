//! Bounded GitHub/Vercel metadata requests; credentials never enter the graph.
use {
    crate::publication::document::publication_inventory::model::Version,
    anyhow::{Result, ensure},
    futures::StreamExt,
    serde::Deserialize,
    sha2::{Digest, Sha256},
    std::collections::{BTreeMap, HashSet},
};

#[derive(Deserialize)]
pub(super) struct Deployment {
    url:     String,
    created: u64,
    meta:    Option<Metadata>,
}
#[derive(Deserialize)]
struct Metadata {
    #[serde(rename = "githubCommitRef")]
    branch: Option<String>,
}
#[derive(Deserialize)]
pub(super) struct PullRequest {
    head: Head,
}
#[derive(Deserialize)]
struct Head {
    #[serde(rename = "ref")]
    branch: String,
    repo:   Option<Repository>,
}
#[derive(Deserialize)]
struct Repository {
    full_name: String,
}
#[derive(Deserialize)]
struct Page {
    deployments: Vec<Deployment>,
    pagination:  Pagination,
}
#[derive(Deserialize)]
struct Pagination {
    next: Option<u64>,
}

pub(super) struct Api {
    pub github_token: String,
    pub vercel_token: String,
    pub project_id:   String,
    pub team_id:      String,
}
impl Api {
    pub fn environment() -> Result<Self> {
        fn required(name: &str) -> Result<String> {
            let value = std::env::var(name)
                .map_err(|_| anyhow::anyhow!("missing {name}"))?;
            ensure!(!value.trim().is_empty(), "empty {name}");
            Ok(value)
        }
        Ok(Self {
            github_token: required("GITHUB_TOKEN")?,
            vercel_token: required("VERCEL_TOKEN")?,
            project_id:   required("VERCEL_PROJECT_ID")?,
            team_id:      required("VERCEL_ORG_ID")?,
        })
    }

    pub async fn observe(
        &self,
        repository: &str,
        path: &str,
        hidden: &[String],
        reserved: &[String],
    ) -> Result<(Vec<Version>, String)> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()?;
        let mut hash = Sha256::new();
        let mut pulls = Vec::new(); // bounded: 1000 PRs, pages capped below.
        for page in 1..=10 {
            let url = format!("https://api.github.com/repos/{repository}/pulls?state=open&per_page=100&page={page}");
            let bytes = body(
                client
                    .get(url)
                    .bearer_auth(&self.github_token)
                    .header("Accept", "application/vnd.github+json")
                    .header("X-GitHub-Api-Version", "2022-11-28")
                    .header("User-Agent", "GraphArch"),
            )
            .await?;
            hash.update(&bytes);
            let values: Vec<PullRequest> = serde_json::from_slice(&bytes)?;
            ensure!(
                values.len() <= 100,
                "GitHub PR page exceeds requested limit"
            );
            let done = values.len() < 100;
            pulls.extend(values);
            if done {
                break;
            }
            ensure!(
                page < 10,
                "GitHub PR inventory exceeds 1000-record budget"
            );
        }
        let mut deployments = Vec::new(); // bounded: 1000 deployments.
        let mut cursor = None;
        for page in 0..10 {
            let mut url =
                url::Url::parse("https://api.vercel.com/v7/deployments")?;
            url.query_pairs_mut()
                .append_pair("projectId", &self.project_id)
                .append_pair("teamId", &self.team_id)
                .append_pair("state", "READY")
                .append_pair("limit", "100");
            if let Some(value) = cursor {
                url.query_pairs_mut()
                    .append_pair("until", &format!("{value}"));
            }
            let bytes =
                body(client.get(url).bearer_auth(&self.vercel_token)).await?;
            hash.update(&bytes);
            let response: Page = serde_json::from_slice(&bytes)?;
            ensure!(
                response.deployments.len() <= 100,
                "Vercel page exceeds requested limit"
            );
            deployments.extend(response.deployments);
            match response.pagination.next {
                None => break,
                Some(next) => {
                    ensure!(
                        cursor.is_none_or(|old| next < old),
                        "Vercel pagination did not advance"
                    );
                    ensure!(
                        page < 9,
                        "Vercel inventory exceeds 1000-record budget"
                    );
                    cursor = Some(next);
                },
            }
        }
        Ok((
            discover(
                deployments,
                pulls,
                repository,
                path,
                hidden,
                reserved,
            )?,
            hex::encode(hash.finalize()),
        ))
    }
}

async fn body(request: reqwest::RequestBuilder) -> Result<Vec<u8>> {
    let response = request.send().await?.error_for_status()?;
    ensure!(
        response.status().is_success(),
        "publication API requires successful response"
    );
    ensure!(
        response
            .content_length()
            .is_none_or(|n| n <= 2 * 1024 * 1024),
        "publication API page exceeds 2 MiB"
    );
    let mut bytes = Vec::new(); // bounded: each response capped at 2 MiB.
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        ensure!(
            bytes.len() + chunk.len() <= 2 * 1024 * 1024,
            "publication API page exceeds 2 MiB"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

pub(super) fn discover(
    deployments: Vec<Deployment>,
    pulls: Vec<PullRequest>,
    repository: &str,
    path: &str,
    hidden: &[String],
    reserved: &[String],
) -> Result<Vec<Version>> {
    let open: HashSet<_> = pulls
        .into_iter()
        .filter(|p| {
            p.head
                .repo
                .as_ref()
                .is_some_and(|r| r.full_name.eq_ignore_ascii_case(repository))
        })
        .map(|p| p.head.branch)
        .collect(); // bounded: input page budgets.
    let mut latest = BTreeMap::<String, Deployment>::new(); // bounded: input page budgets.
    for deployment in deployments {
        let Some(branch) = deployment
            .meta
            .as_ref()
            .and_then(|meta| meta.branch.as_ref())
        else {
            continue
        }; // Manual deployments have no repository branch.
        if reserved.contains(branch) {
            continue;
        }
        let replace = latest.get(branch).is_none_or(|old| {
            (deployment.created, &deployment.url) > (old.created, &old.url)
        });
        if replace {
            latest.insert(branch.clone(), deployment);
        }
    }
    let mut slugs = HashSet::new(); // bounded: distinct branches above.
    let mut versions = Vec::new(); // bounded: distinct branches above.
    for (branch, deployment) in latest {
        let slug = branch.replace('/', "-");
        ensure!(
            slugs.insert(slug.clone()),
            "publication branch slugs collide"
        );
        ensure!(
            !deployment.url.contains('/') &&
                deployment.url.ends_with(".vercel.app"),
            "Vercel deployment must name an immutable vercel.app host"
        );
        let version = Version {
            slug,
            advertised: open.contains(&branch) &&
                !hidden.iter().any(|p| branch.starts_with(p)),
            label: branch.clone(),
            branch,
            url: format!("https://{}/{path}", deployment.url),
        };
        version.validate()?;
        versions.push(version);
    }
    Ok(versions)
}

#[cfg(test)]
mod http_tests {
    use {
        super::body,
        tokio::io::{AsyncReadExt, AsyncWriteExt},
    };

    async fn response(status: &str, contents: &str) -> reqwest::RequestBuilder {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: \
             close\r\n\r\n{contents}",
            contents.len()
        );
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut request = [0; 2048];
            stream.read(&mut request).await.unwrap();
            stream.write_all(response.as_bytes()).await.unwrap();
        });
        reqwest::Client::new().get(format!("http://{address}/"))
    }

    #[tokio::test]
    async fn unsuccessful_api_responses_never_become_empty_inventories() {
        let request =
            response("403 Forbidden", "{\"message\":\"denied\"}").await;
        assert!(body(request).await.is_err());
        let request = response("200 OK", "[]").await;
        assert_eq!(body(request).await.unwrap(), b"[]");
    }
}
