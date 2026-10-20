//! Line breaks at the TextBox boundary. The session's text breaks lines with `\n`; a system XAML
//! TextBox keeps `\r` (it turns `\n` and `\r\n` into `\r`, and one without `AcceptsReturn` keeps only
//! the first line). Text going into a field is written in the TextBox's form, text read back is
//! returned in the session's, so an untouched field reads back exactly what the session holds.

pub struct LineBreaks;

impl LineBreaks {
    /// The session's text as a TextBox holds it.
    pub fn shown(text: &str) -> String {
        Self::typed(text).replace('\n', "\r")
    }

    /// A TextBox's text as the session holds it.
    pub fn typed(text: &str) -> String {
        text.replace("\r\n", "\n").replace('\r', "\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_textbox_form_breaks_lines_with_cr() {
        assert_eq!(LineBreaks::shown("a\nb\n\nc\n"), "a\rb\r\rc\r");
        assert_eq!(LineBreaks::shown("a\r\nb"), "a\rb");
        assert_eq!(LineBreaks::shown("one line"), "one line");
    }

    #[test]
    fn read_back_text_breaks_lines_with_lf() {
        assert_eq!(LineBreaks::typed("a\rb\r\rc\r"), "a\nb\n\nc\n");
        assert_eq!(LineBreaks::typed("a\r\nb\nc"), "a\nb\nc");
        assert_eq!(LineBreaks::typed(""), "");
    }

    #[test]
    fn a_round_trip_keeps_the_sessions_text() {
        for text in [
            "",
            "a",
            "a\n",
            "a\nb",
            "\n\na\n\n",
            "contoso.example\nshop-contoso.example\n",
        ] {
            assert_eq!(LineBreaks::typed(&LineBreaks::shown(text)), text);
        }
    }
}
