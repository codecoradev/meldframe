//! Where the server keeps the assets.

/// The directory of the assets (`MELDFRAME_ASSETS_DIR`, ADR-0015): required in a release build, so images are
/// never written to a place nobody chose; a development build keeps them in `.dev-assets`.
pub fn assets_dir(release: bool, set: Option<&str>) -> Result<String, Box<dyn std::error::Error>> {
    match (set.map(str::trim).filter(|d| !d.is_empty()), release) {
        (Some(dir), _) => Ok(dir.to_owned()),
        (None, false) => Ok(".dev-assets".to_owned()),
        (None, true) => {
            Err("set MELDFRAME_ASSETS_DIR to the directory that keeps the assets".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::assets_dir;

    #[test]
    fn a_release_build_needs_the_assets_directory_and_a_development_build_has_a_default() {
        assert_eq!(assets_dir(false, None).unwrap(), ".dev-assets");
        assert_eq!(assets_dir(false, Some("  ")).unwrap(), ".dev-assets");
        assert!(assets_dir(true, None).is_err());
        assert!(assets_dir(true, Some("")).is_err());
        assert_eq!(
            assets_dir(true, Some("/srv/assets")).unwrap(),
            "/srv/assets"
        );
    }
}
