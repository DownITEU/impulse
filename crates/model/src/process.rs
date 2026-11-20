//! The game's process: which one it is, and how a trainer says which to look for.

use std::{
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Clone, Debug, PartialEq)]
pub struct Process {
    pub pid: u32,
    pub path: PathBuf,
    /// Creation time in 100 ns ticks. With the pid it names this process alone: a reused pid has
    /// another.
    pub created: u64,
}

/// Selects a game process by executable path or install folder and a minimum process age.
#[derive(Clone)]
pub struct Rule {
    /// Path suffix of the executable, `*` matching inside one folder or file name. Without it
    /// the install folder decides.
    pub image: Option<String>,
    pub location: PathBuf,
    /// A younger process is still starting up.
    pub minimum_age: Duration,
}

impl Rule {
    pub fn matches(&self, path: &Path) -> bool {
        let path = path.to_string_lossy().to_lowercase();
        match &self.image {
            Some(image) => ends_with_image(&path, &image.to_lowercase()),
            None => {
                let folder = self.location.to_string_lossy().to_lowercase();
                path.strip_prefix(folder.trim_end_matches('\\')).is_some_and(|rest| rest.starts_with('\\'))
            }
        }
    }
}

/// `path` ends with the folders and file of `image`, whole names only: `game.exe` is not `mygame.exe`.
fn ends_with_image(path: &str, image: &str) -> bool {
    let mut names = path.rsplit('\\');
    let patterns = image.rsplit(['\\', '/']);
    patterns.into_iter().all(|pattern| names.next().is_some_and(|name| glob(pattern, name)))
}

/// `*` matches any run of characters. Iterative, at most pattern x text steps: the pattern is
/// remote data, and recursion would let it stall the search or the stack.
fn glob(pattern: &str, text: &str) -> bool {
    let (pattern, text): (Vec<char>, Vec<char>) = (pattern.chars().collect(), text.chars().collect());
    let (mut at, mut seen) = (0, 0);
    // Where the last `*` was and how much text it has taken so far.
    let mut star = None;
    while seen < text.len() {
        if pattern.get(at) == Some(&'*') {
            star = Some((at + 1, seen));
            at += 1;
        } else if pattern.get(at) == Some(&text[seen]) {
            at += 1;
            seen += 1;
        } else if let Some((resume, taken)) = star {
            (at, seen, star) = (resume, taken + 1, Some((resume, taken + 1)));
        } else {
            return false;
        }
    }
    pattern[at..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_matches_whole_trailing_names() {
        let path = r"d:\steam\common\enshrouded\enshrouded.exe";
        assert!(ends_with_image(path, "enshrouded.exe"));
        assert!(ends_with_image(path, r"enshrouded\enshrouded.exe"));
        assert!(ends_with_image(path, "ens*ded.exe"));
        assert!(ends_with_image(path, "*.exe"));
        assert!(!ends_with_image(path, "shrouded.exe"), "a longer name is another program");
        assert!(!ends_with_image(path, r"*.exe\enshrouded.exe"));
        assert!(!ends_with_image(r"c:\a.exe", r"b\a.exe"), "more names than the path has");
        assert!(!ends_with_image(path, "enshrouded.ex"));
    }

    #[test]
    fn a_hostile_pattern_cannot_stall_the_search() {
        let started = std::time::Instant::now();
        let pattern = format!("{}b", "*a".repeat(2000));
        assert!(!ends_with_image(&format!(r"c:\{}", "a".repeat(200)), &pattern));
        assert!(ends_with_image(r"c:\aaab", "*a*a*b"));
        assert!(started.elapsed() < Duration::from_secs(2), "{:?}", started.elapsed());
    }

    #[test]
    fn without_an_image_the_install_folder_decides() {
        let rule = Rule { image: None, location: PathBuf::from(r"D:\Steam\common\Game\"), minimum_age: Duration::ZERO };
        assert!(rule.matches(Path::new(r"D:\STEAM\common\game\bin\x64\g.exe")));
        assert!(!rule.matches(Path::new(r"D:\Steam\common\Game2\g.exe")), "a sibling folder with the same prefix");
        assert!(!rule.matches(Path::new(r"D:\Steam\common\g.exe")));
    }
}
