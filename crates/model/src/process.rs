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

/// How a trainer names its game's process, wherever the game is installed.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    /// Path suffix of the executable, `*` matching inside one folder or file name. Without it
    /// the install folder decides.
    pub image: Option<String>,
    /// A younger process is still starting up.
    pub minimum_age: Duration,
    pub pick: Pick,
    /// Text the process's command line holds, after the program's name when quoted; case-sensitive.
    pub arguments: Option<String>,
}

/// Which of several matching processes is the game.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Pick {
    /// The one with the most memory.
    #[default]
    Biggest,
    Oldest,
    Newest,
}

/// Selects a game process: the trainer's `Target` in this install.
#[derive(Clone)]
pub struct Rule {
    pub target: Target,
    pub location: PathBuf,
    /// Without an image, these folders count as the install's too (`Installation::alternates`).
    pub alternates: Vec<PathBuf>,
}

impl Rule {
    pub fn matches(&self, path: &Path) -> bool {
        let path = path.to_string_lossy().to_lowercase();
        let inside = |folder: &PathBuf| {
            // Steam's own registry path has forward slashes.
            let folder = folder.to_string_lossy().replace('/', "\\").to_lowercase();
            path.strip_prefix(folder.trim_end_matches('\\')).is_some_and(|rest| rest.starts_with('\\'))
        };
        match &self.target.image {
            Some(image) => ends_with_image(&path, &image.to_lowercase()),
            None => inside(&self.location) || self.alternates.iter().any(inside),
        }
    }
}

impl Target {
    /// Whether `command_line` holds the arguments: a quoted program name cut, the rest trimmed.
    pub fn takes(&self, command_line: &str) -> bool {
        let Some(arguments) = &self.arguments else {
            return true;
        };
        let rest = command_line.strip_prefix('"').and_then(|quoted| quoted.split_once('"')).map_or(command_line, |(_, rest)| rest.trim());
        rest.contains(arguments.as_str())
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
    fn without_an_image_the_install_folders_decide() {
        let location = PathBuf::from(r"D:\Steam\common\Game\");
        let target = Target { image: None, minimum_age: Duration::ZERO, pick: Pick::Biggest, arguments: None };
        let rule = Rule { target, location, alternates: vec![PathBuf::from(r"E:\XboxGames\Game\Content")] };
        assert!(rule.matches(Path::new(r"D:\STEAM\common\game\bin\x64\g.exe")));
        assert!(rule.matches(Path::new(r"E:\XboxGames\Game\Content\g.exe")), "an alternate folder");
        assert!(!rule.matches(Path::new(r"D:\Steam\common\Game2\g.exe")), "a sibling folder with the same prefix");
        assert!(!rule.matches(Path::new(r"D:\Steam\common\g.exe")));
        let slashed = Rule { location: PathBuf::from("c:/program files (x86)/steam/steamapps/common/Game"), ..rule };
        assert!(slashed.matches(Path::new(r"C:\Program Files (x86)\Steam\steamapps\common\Game\g.exe")), "Steam's own slashes");
    }

    #[test]
    fn the_arguments_are_looked_for_after_a_quoted_program_name() {
        let target =
            |arguments: &str| Target { image: None, minimum_age: Duration::ZERO, pick: Pick::Biggest, arguments: Some(arguments.into()) };
        assert!(target("-server").takes(r#""C:\g\game -server.exe"  -server -port 1"#));
        assert!(!target("-server").takes(r#""C:\g\game -server.exe" -client"#), "the program's own name does not count");
        assert!(target("game.exe").takes(r"C:\g\game.exe -client"), "an unquoted name is searched too");
        assert!(!target("-Server").takes(r#""g.exe" -server"#), "case-sensitive");
        assert!(Target { arguments: None, ..target("") }.takes(""));
    }
}
