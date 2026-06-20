/// Partial-text include/exclude filters for process command lines.
///
/// Matching is case-insensitive: patterns are stored pre-lowercased (see
/// [`Filters::new`]) and candidates are lowercased before comparison.
pub struct Filters {
    includes: Vec<String>,
    excludes: Vec<String>,
}

impl Filters {
    /// Build filters from raw include/exclude patterns, lowercasing each so that
    /// matching is case-insensitive.
    pub fn new(includes: &[String], excludes: &[String]) -> Self {
        Filters {
            includes: includes.iter().map(|s| s.to_lowercase()).collect(),
            excludes: excludes.iter().map(|s| s.to_lowercase()).collect(),
        }
    }

    /// A line is kept when no exclude matches AND (there are no includes, or at
    /// least one include matches). Exclude takes precedence over include.
    pub fn keep(&self, command_line: &str) -> bool {
        let haystack = command_line.to_lowercase();
        if self.excludes.iter().any(|e| haystack.contains(e)) {
            return false;
        }
        self.includes.is_empty() || self.includes.iter().any(|i| haystack.contains(i))
    }
}

#[cfg(test)]
mod tests {
    use super::Filters;

    fn filters(includes: &[&str], excludes: &[&str]) -> Filters {
        let inc: Vec<String> = includes.iter().map(|s| s.to_string()).collect();
        let exc: Vec<String> = excludes.iter().map(|s| s.to_string()).collect();
        Filters::new(&inc, &exc)
    }

    #[test]
    fn no_filters_keeps_everything() {
        assert!(filters(&[], &[]).keep("anything at all"));
    }

    #[test]
    fn include_only_keeps_matching_partial_text() {
        let f = filters(&["notepad"], &[]);
        assert!(f.keep(r"C:\Windows\notepad.exe foo.txt"));
        assert!(!f.keep(r"C:\Windows\calc.exe"));
    }

    #[test]
    fn matching_is_case_insensitive() {
        assert!(filters(&["NOTEPAD"], &[]).keep(r"c:\windows\Notepad.exe"));
    }

    #[test]
    fn exclude_overrides_include() {
        let f = filters(&["windows"], &["notepad"]);
        assert!(f.keep(r"C:\Windows\explorer.exe"));
        assert!(!f.keep(r"C:\Windows\notepad.exe")); // excluded despite include match
    }

    #[test]
    fn exclude_only_keeps_everything_else() {
        let f = filters(&[], &["svchost"]);
        assert!(f.keep(r"C:\app\foo.exe"));
        assert!(!f.keep(r"C:\Windows\System32\svchost.exe -k netsvcs"));
    }

    #[test]
    fn multiple_includes_are_or_ed() {
        let f = filters(&["notepad", "calc"], &[]);
        assert!(f.keep(r"C:\Windows\notepad.exe"));
        assert!(f.keep(r"C:\Windows\calc.exe"));
        assert!(!f.keep(r"C:\Windows\mspaint.exe"));
    }

    #[test]
    fn multiple_excludes_are_or_ed() {
        let f = filters(&[], &["svchost", "conhost"]);
        assert!(!f.keep(r"C:\Windows\System32\svchost.exe"));
        assert!(!f.keep(r"C:\Windows\System32\conhost.exe"));
        assert!(f.keep(r"C:\app\foo.exe"));
    }
}
