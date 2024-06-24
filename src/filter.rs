use std::borrow::Cow;

use glob_match::glob_match;

type FilterListType<'src> = Vec<Cow<'src, str>>;

/// Specifies a set of filters for which objects should be included and which should be excluded.
pub struct Filter<'src> {
    included: FilterListType<'src>,
    excluded: FilterListType<'src>,
}

impl<'src> Filter<'src> {
    /// Creates a new [Filter] from the given lists of `included` and `excluded` glob strings.
    ///
    /// - If both `included` and `excluded` globs are specified, files will only be included if they match both sets.
    /// - If only `included` is specified and `excluded` is empty, `included` acts as a whitelist.
    /// - If only `excluded` is specified and `included` is empty, `excluded` acts as a blacklist.
    /// - If both are empty, the filter passes for all paths (and you should call [all](Filter::all) instead).
    pub fn new<Iter, Str>(included: Iter, excluded: Iter) -> Filter<'src>
    where
        Iter: IntoIterator<Item = Str>,
        Str: Into<Cow<'src, str>>,
    {
        Filter {
            included: included.into_iter().map(|s| s.into()).collect(),
            excluded: excluded.into_iter().map(|s| s.into()).collect(),
        }
    }

    /// Creates a new filter that passes for all paths.
    pub fn all() -> Filter<'src> {
        Filter {
            included: Vec::new(),
            excluded: Vec::new(),
        }
    }

    /// Returns whether the given path matches this filter.
    pub fn check(&self, path: &str) -> bool {
        let is_included = self.match_path(&self.included, path).unwrap_or(true);
        let is_excluded = self.match_path(&self.excluded, path).unwrap_or(false);

        return is_included && !is_excluded;
    }

    /// Returns whether a path matches the given glob array.
    fn match_path(&self, globs: &FilterListType, path: &str) -> Option<bool> {
        if globs.is_empty() {
            return None;
        }

        for glob in globs {
            if glob_match(glob, path) {
                return Some(true);
            }
        }

        return Some(false);
    }
}
