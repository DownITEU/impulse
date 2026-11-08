use std::collections::HashMap;


use super::{
    App,
    registry::{self, CLASSES_ROOT, View},
};

const PACKAGES: &str =
    r"Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";

/// Xbox and Microsoft Store games, keyed `uwp:<package family name>`.
pub fn apps(known: impl Fn(&str) -> bool) -> Vec<App> {
    let Some(packages) = registry::open(CLASSES_ROOT, PACKAGES, View::Wow64) else {
        return Vec::new();
    };
    let Ok(names) = packages.keys() else {
        return Vec::new();
    };
    // A family also holds resource packs and bundles; the main package wins, x64 first.
    let mut found: HashMap<String, ((bool, bool), String)> = HashMap::new();
    for full in names {
        let Some((family, arch, resource)) = parse(&full) else {
            continue;
        };
        let store_id = format!("uwp:{family}");
        let rank = (resource.is_empty(), arch == "x64");
        if !known(&store_id)
            || found.get(&store_id).is_some_and(|(had, _)| *had >= rank)
        {
            continue;
        }
        let root = registry::open(&packages, &full, View::Wow64)
            .and_then(|key| registry::string(&key, "PackageRootFolder"));
        if let Some(root) = root {
            found.insert(store_id, (rank, root));
        }
    }
    found
        .into_iter()
        .map(|(store_id, (_, root))| App {
            store_id,
            location: root.into(),
        })
        .collect()
}

// Full name: name_version_arch_resourceId_publisherId; family: name_publisherId.
fn parse(full: &str) -> Option<(String, &str, &str)> {
    let parts: Vec<&str> = full.split('_').collect();
    let [name, _, arch, resource, publisher] = parts[..] else {
        return None;
    };
    Some((format!("{name}_{publisher}"), arch, resource))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_package_full_name() {
        let (family, arch, resource) =
            parse("Microsoft.Chelan_1.3385.0.0_x64__8wekyb3d8bbwe").unwrap();
        assert_eq!(
            (family.as_str(), arch, resource),
            ("Microsoft.Chelan_8wekyb3d8bbwe", "x64", "")
        );
        let (_, _, pack) = parse("Game_1.0.0.0_neutral_split.scale-100_pub").unwrap();
        assert_eq!(pack, "split.scale-100");
        assert!(parse("not-a-package").is_none());
    }
}
