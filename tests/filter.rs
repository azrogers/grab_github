use grab_github::Filter;

#[test]
pub fn only_included() {
    let filter = Filter::new(vec!["src/**", "test", "*/any", "other/*"], vec![]);

    assert!(filter.check("src/test.rs"));
    assert!(filter.check("src/other"));
    assert!(filter.check("src/"));
    assert!(filter.check("src/test/test"));
    assert!(filter.check("test"));
    assert!(filter.check("one/any"));
    assert!(filter.check("two/any"));
    assert!(filter.check("other/one"));
    assert!(!filter.check("src"));
    assert!(!filter.check("wrong/test.rs"));
    assert!(!filter.check("wrong/test"));
    assert!(!filter.check("wrong.rs"));
    assert!(!filter.check("other/one/two"));
    assert!(!filter.check("one/any/other"));
}

#[test]
pub fn only_excluded() {
    let filter = Filter::new(vec![], vec!["src/**", "test", "*/any", "other/*"]);

    assert!(!filter.check("src/test.rs"));
    assert!(!filter.check("src/other"));
    assert!(!filter.check("src/"));
    assert!(!filter.check("src/test/test"));
    assert!(!filter.check("test"));
    assert!(!filter.check("one/any"));
    assert!(!filter.check("two/any"));
    assert!(!filter.check("other/one"));
    assert!(filter.check("src"));
    assert!(filter.check("wrong/test.rs"));
    assert!(filter.check("wrong/test"));
    assert!(filter.check("wrong.rs"));
    assert!(filter.check("other/one/two"));
    assert!(filter.check("one/any/other"));
}

#[test]
pub fn both() {
    let filter = Filter::new(
        vec!["src/**", "test", "*/any", "other/*"],
        vec!["**/other", "one/*", "src/test/test"],
    );

    assert!(filter.check("src/test.rs"));
    assert!(filter.check("src/"));
    assert!(filter.check("test"));
    assert!(filter.check("two/any"));
    assert!(filter.check("other/one"));
    assert!(!filter.check("one/any"));
    assert!(!filter.check("src/test/test"));
    assert!(!filter.check("src/other"));
    assert!(!filter.check("src"));
    assert!(!filter.check("wrong/test.rs"));
    assert!(!filter.check("wrong/test"));
    assert!(!filter.check("wrong.rs"));
    assert!(!filter.check("other/one/two"));
    assert!(!filter.check("one/any/other"));
}

#[test]
pub fn neither() {
    let filter = Filter::all();

    assert!(filter.check("src/test.rs"));
    assert!(filter.check("src/"));
    assert!(filter.check("test"));
    assert!(filter.check("two/any"));
    assert!(filter.check("other/one"));
    assert!(filter.check("one/any"));
    assert!(filter.check("src/test/test"));
    assert!(filter.check("src/other"));
    assert!(filter.check("src"));
    assert!(filter.check("wrong/test.rs"));
    assert!(filter.check("wrong/test"));
    assert!(filter.check("wrong.rs"));
    assert!(filter.check("other/one/two"));
    assert!(filter.check("one/any/other"));
}
