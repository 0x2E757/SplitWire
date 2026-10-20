/// XML text and the namespaces of a loaded document.
pub struct Xml;

impl Xml {
    pub const NAMESPACES: &'static str = r#"xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation" xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml""#;

    pub fn escape(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&apos;")
            .replace('\n', "&#10;")
            .replace('\r', "&#13;")
    }

    /// `markup` with the namespaces on its first element.
    pub fn root(markup: &str) -> String {
        let markup = markup.trim_start();
        match markup.find([' ', '>', '/']) {
            Some(end) if markup.starts_with('<') => {
                format!("{} {}{}", &markup[..end], Self::NAMESPACES, &markup[end..])
            }
            _ => markup.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_and_roots() {
        assert_eq!(
            Xml::escape("a<\"b\">&'\n"),
            "a&lt;&quot;b&quot;&gt;&amp;&apos;&#10;"
        );
        assert!(Xml::root("<Grid/>").starts_with("<Grid xmlns="));
        assert!(Xml::root("<Grid>x</Grid>").ends_with("x</Grid>"));
    }
}
