use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use super::{
    App,
    registry::{self, CLASSES_ROOT, LOCAL_MACHINE, View},
    same_path,
};

const PACKAGES: &str = r"Local Settings\Software\Microsoft\Windows\CurrentVersion\AppModel\Repository\Packages";
const PACKAGE_DATA: &str = r"SOFTWARE\Microsoft\Windows\CurrentVersion\AppModel\StateRepository\Cache\Package\Data";

/// A family's package: it also holds resource packs and bundles.
struct Package {
    /// The main package wins, x64 first: (main, x64).
    rank: (bool, bool),
    full: String,
    root: String,
}

/// Xbox and Microsoft Store games, keyed `uwp:<package family name>`.
pub fn apps(known: impl Fn(&str) -> bool) -> Vec<App> {
    let Some(packages) = registry::open(CLASSES_ROOT, PACKAGES, View::Wow64) else {
        return Vec::new();
    };
    let Ok(names) = packages.keys() else {
        return Vec::new();
    };
    let mut found: HashMap<String, Package> = HashMap::new();
    for full in names {
        let Some((family, arch, resource)) = parse(&full) else {
            continue;
        };
        let store_id = format!("uwp:{family}");
        let rank = (resource.is_empty(), arch == "x64");
        if !known(&store_id) || found.get(&store_id).is_some_and(|had| had.rank >= rank) {
            continue;
        }
        let root = registry::open(&packages, &full, View::Wow64).and_then(|key| registry::string(&key, "PackageRootFolder"));
        if let Some(root) = root {
            found.insert(store_id, Package { rank, full, root });
        }
    }
    let mut folders = package_folders(found.values().map(|package| package.full.to_lowercase()).collect());
    let apps = found.into_iter().map(|(store_id, Package { full, root, .. })| {
        let location = PathBuf::from(root);
        let mut app = App::new(store_id, location.clone());
        app.alternates = folders.remove(&full.to_lowercase()).unwrap_or_default();
        app.alternates.retain(|folder| !same_path(folder, &location));
        app
    });
    apps.collect()
}

/// Each package's other folders by lower-cased full name: a GDK game may run from its `InstalledLocation`
/// or a mutable copy rather than the package root.
fn package_folders(wanted: HashSet<String>) -> HashMap<String, Vec<PathBuf>> {
    let Some(data) = registry::open(LOCAL_MACHINE, PACKAGE_DATA, View::Wow64) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (_, key) in registry::children(&data, View::Wow64) {
        let Some(full) = registry::string(&key, "PackageFullName").map(|full| full.to_lowercase()).filter(|full| wanted.contains(full))
        else {
            continue;
        };
        let mut folders: Vec<PathBuf> = Vec::new();
        for name in ["InstalledLocation", "MutableLink", "MutableLocation"] {
            let Some(folder) = registry::string(&key, name) else { continue };
            let folder = PathBuf::from(folder.replace('/', "\\").trim_end_matches('\\'));
            if !folders.iter().any(|had| same_path(had, &folder)) {
                folders.push(folder);
            }
        }
        out.insert(full, folders);
    }
    out
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
        let (family, arch, resource) = parse("Microsoft.Chelan_1.3385.0.0_x64__8wekyb3d8bbwe").unwrap();
        assert_eq!((family.as_str(), arch, resource), ("Microsoft.Chelan_8wekyb3d8bbwe", "x64", ""));
        let (_, _, pack) = parse("Game_1.0.0.0_neutral_split.scale-100_pub").unwrap();
        assert_eq!(pack, "split.scale-100");
        assert!(parse("not-a-package").is_none());
    }

    #[test]
    fn package_folders_of_this_machine_read_without_failing() {
        // Whatever is installed: only the read is checked, and that an unwanted package is not.
        assert!(package_folders(HashSet::from(["no.such.package_1.0.0.0_x64__x".into()])).is_empty());
    }
}
