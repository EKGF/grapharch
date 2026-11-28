//! The artwork notices shipped with every publication that may use icons.
use anyhow::{Result, ensure};

pub(crate) async fn write_licenses(root: &std::path::Path) -> Result<String> {
    const NAME: &str = "grapharch-icon-licenses.txt";
    let path = root.join(NAME);
    ensure!(
        !tokio::fs::try_exists(&path).await?,
        "icon license artifact collides with publication output"
    );
    let notices = concat!(
        "GraphArch bundled icons\n\nLucide / Feather\n",
        include_str!("../render/website/project/icons/LICENSE"),
        "\nMaterial Design Icons (Pictogrammers), @mdi/svg 7.4.47\n",
        include_str!("../render/website/project/icons/sets/LICENSE-mdi"),
        "\n",
        include_str!("../render/website/project/icons/sets/LICENSE-APACHE-2.0"),
        "\nOcticons (GitHub), @primer/octicons 19.43.0\n",
        include_str!("../render/website/project/icons/sets/LICENSE-octicons"),
        "\nFont Awesome Free 6.7.2 by Fonticons, Inc. \
         (https://fontawesome.com), icons under CC BY 4.0 \
         (https://creativecommons.org/licenses/by/4.0/)\n",
        include_str!(
            "../render/website/project/icons/sets/LICENSE-fontawesome"
        ),
        "\nSimple Icons 16.38.1 (https://simpleicons.org), CC0 1.0\n",
        include_str!(
            "../render/website/project/icons/sets/LICENSE-simple-icons"
        ),
        "\n",
        include_str!(
            "../render/website/project/icons/sets/DISCLAIMER-simple-icons"
        ),
    );
    tokio::fs::write(path, notices).await?;
    Ok(NAME.into())
}
