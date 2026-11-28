//! An MkDocs site styled by Material and EKGF's `material-ekgf` plugin is
//! presented like the live site: the EKGF documentation theme selected through
//! the RDF mapping table, Material features mapped to the page layout, the
//! palette as theme tokens, Git page dates and the EKGF site footer.
use {
    anyhow::Result,
    std::{path::Path, process::Command},
};

fn write(root: &Path, path: &str, body: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn git(root: &Path, args: &[&str], date: Option<&str>) {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    if let Some(date) = date {
        command
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date);
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

const MATERIAL_EKGF: &str = r#"site_name: Orchard Principles
site_url: https://orchard.example.org/
repo_url: https://github.com/example/orchard
copyright: Copyright © 2026 Orchard Association
theme:
  name: material
  palette:
    - media: "(prefers-color-scheme: light)"
      scheme: default
      primary: indigo
      accent: light-blue
      toggle:
        icon: material/weather-night
        name: Switch to dark mode
    - media: "(prefers-color-scheme: dark)"
      scheme: slate
      primary: indigo
      accent: deep orange
      toggle:
        icon: material/weather-sunny
        name: Switch to light mode
  features:
    - navigation.tabs
    - navigation.indexes
    - toc.integrate
extra:
  generator: false
  social:
    - icon: fontawesome/brands/twitter
      link: https://twitter.com/orchard
    - icon: fontawesome/brands/github
      link: https://github.com/example/orchard
plugins:
  - material-ekgf
  - git-revision-date-localized:
      enable_creation_date: true
markdown_extensions:
  - pymdownx.smartsymbols
"#;

/// A two-commit repository: `guide/pruning.md` changes in the second.
fn material_ekgf_site(root: &Path, configuration: &str) {
    write(root, "mkdocs.yml", configuration);
    write(
        root,
        "docs/index.md",
        "---\nhide:\n  - navigation\n  - toc\n---\n# Manifesto\n\nMany---if \
         not most--pages (c) 2026 --> \
         [pruning](guide/pruning.md).\n\n`a---b`\n",
    );
    write(
        root,
        "docs/guide/index.md",
        "# Guide\n\nStart here.\n",
    );
    write(
        root,
        "docs/guide/pruning.md",
        "# Pruning\n\n## Winter\n\nCut.\n",
    );
    git(root, &["init", "-q", "-b", "main"], None);
    git(root, &["config", "user.name", "Ada"], None);
    git(
        root,
        &["config", "user.email", "a@example.org"],
        None,
    );
    git(root, &["add", "."], None);
    git(
        root,
        &["commit", "-q", "-m", "one"],
        Some("2022-03-04T05:06:07Z"),
    );
    write(
        root,
        "docs/guide/pruning.md",
        "# Pruning\n\n## Winter\n\nCut back.\n",
    );
    git(root, &["add", "."], None);
    git(
        root,
        &["commit", "-q", "-m", "two"],
        Some("2025-06-07T08:09:10Z"),
    );
}

fn build(root: &Path) -> std::path::PathBuf {
    let output = Command::new(env!("CARGO_BIN_EXE_grapharch"))
        .arg("build")
        .arg(root)
        .arg("--output")
        .arg(root.join("output/grapharch"))
        .arg("--offline")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    root.join("output/grapharch/site")
}

fn page(site: &Path, path: &str) -> String {
    std::fs::read_to_string(site.join(path)).unwrap()
}

#[test]
fn material_ekgf_selects_the_ekgf_documentation_theme() -> Result<()> {
    let root = tempfile::tempdir()?;
    material_ekgf_site(root.path(), MATERIAL_EKGF);
    let site = build(root.path());
    let home = page(&site, "index.html");
    assert!(home.contains("class=\"theme-ekgf ekgf-documentation\""));
    // EKGF logo at the start, the centred site title, the OMG mark at the end.
    let logo = home.find("class=\"brand-identity\"").expect("brand logo");
    let title = home.find("class=\"site-title\"").expect("site title");
    let mark = home.find("class=\"identity-mark\"").expect("identity mark");
    assert!(logo < title && title < mark);
    assert!(home.contains(">Orchard Principles</a>"));
    // The EKGF community header links belong to ekgf.org, not to this site.
    assert!(!home.contains("class=\"header-navigation\""));
    // Search and the dark-mode toggle; the repository lives in the footer.
    assert!(home.contains("color-scheme-toggle"));
    assert!(home.contains("class=\"site-search-open\""));
    assert!(!home.contains("class=\"source-link\""));
    // The version chip never names a local GraphArch build.
    assert!(!home.contains("dirty"));
    assert!(!home.contains("brand-version"));
    Ok(())
}

#[test]
fn material_features_select_the_navigation_layout() -> Result<()> {
    let root = tempfile::tempdir()?;
    material_ekgf_site(root.path(), MATERIAL_EKGF);
    let site = build(root.path());
    let home = page(&site, "index.html");
    let pruning = page(&site, "guide/pruning/index.html");
    // navigation.tabs: top tabs on every page.
    assert!(home.contains("class=\"primary-navigation\""));
    assert!(pruning.contains("class=\"primary-navigation\""));
    // hide: navigation removes the sidebar; other pages keep it.
    assert!(!home.contains("class=\"site-navigation\""));
    assert!(pruning.contains("class=\"site-navigation\""));
    // toc.integrate puts the outline into the sidebar, not beside the page.
    assert!(!pruning.contains("class=\"page-outline\""));
    assert!(pruning.contains("class=\"integrated-outline\""));
    // No navigation.path: no breadcrumbs. No navigation.footer: no
    // previous/next. extra.generator false: no GraphArch credit.
    for html in [&home, &pruning] {
        assert!(!html.contains("class=\"breadcrumbs\""));
        assert!(!html.contains("page-neighbors"));
        assert!(!html.contains("Generated by"));
    }
    Ok(())
}

#[test]
fn material_options_that_enable_chrome_keep_it() -> Result<()> {
    let root = tempfile::tempdir()?;
    let configuration = MATERIAL_EKGF
        .replace(
            "    - toc.integrate\n",
            "    - navigation.path\n    - navigation.footer\n",
        )
        .replace("  generator: false\n", "  generator: true\n");
    material_ekgf_site(root.path(), &configuration);
    let site = build(root.path());
    let pruning = page(&site, "guide/pruning/index.html");
    assert!(pruning.contains("class=\"breadcrumbs\""));
    assert!(pruning.contains("page-neighbors"));
    assert!(pruning.contains("class=\"page-outline\""));
    assert!(pruning.contains("Generated by"));
    Ok(())
}

#[test]
fn material_palette_and_typography_follow_the_live_site() -> Result<()> {
    let root = tempfile::tempdir()?;
    material_ekgf_site(root.path(), MATERIAL_EKGF);
    let site = build(root.path());
    let style = page(&site, "style.css");
    // Material's indigo primary colours links; deep orange is the dark accent.
    assert!(style.contains("#4051b5"), "indigo primary");
    let home = page(&site, "index.html");
    // pymdownx.smartsymbols plus em and en dashes, never inside code.
    assert!(home.contains("Many—if not most–pages © 2026 →"));
    assert!(home.contains("<code>a---b</code>"));
    Ok(())
}

#[test]
fn pages_show_relative_created_and_updated_dates() -> Result<()> {
    let root = tempfile::tempdir()?;
    material_ekgf_site(root.path(), MATERIAL_EKGF);
    let site = build(root.path());
    let pruning = page(&site, "guide/pruning/index.html");
    let dates = pruning.find("class=\"page-dates\"").expect("page dates");
    let dates = &pruning[dates..];
    let updated = dates
        .find("datetime=\"2025-06-07T08:09:10")
        .expect("updated date");
    let created = dates
        .find("datetime=\"2022-03-04T05:06:07")
        .expect("created date");
    assert!(updated < created);
    assert!(dates.contains("title=\"Last update: 7 June 2025\""));
    assert!(dates.contains("title=\"Created: 4 March 2022\""));
    assert!(pruning.contains("data-relative-time"));
    // The home page never changed after it was created.
    let home = page(&site, "index.html");
    assert!(home.contains("title=\"Created: 4 March 2022\""));
    assert!(home.contains("title=\"Last update: 4 March 2022\""));
    Ok(())
}

#[test]
fn the_ekgf_footer_carries_social_links_copyright_and_licence() -> Result<()> {
    let root = tempfile::tempdir()?;
    material_ekgf_site(root.path(), MATERIAL_EKGF);
    let site = build(root.path());
    let home = page(&site, "index.html");
    let footer = home.find("class=\"site-footer\"").expect("site footer");
    let footer = &home[footer..];
    for text in [
        ">About EKGF</h2>",
        ">Documentation</h2>",
        ">Resources</h2>",
        ">Connect</h2>",
        "href=\"https://twitter.com/orchard\"",
        "aria-label=\"Twitter\"",
        "href=\"https://github.com/example/orchard\"",
        "Copyright © 2026 Orchard Association",
        "class=\"license-badge\"",
        "CC BY-SA 4.0",
        "a Managed Community of the",
    ] {
        assert!(
            footer.contains(text),
            "{text} missing from {footer}"
        );
    }
    // The documentation site lists the EKGF sites, not GraphArch itself.
    assert!(!footer.contains("GraphArch license"));
    Ok(())
}
