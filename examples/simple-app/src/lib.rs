//! A paragraph of text, as lines of words, for an example to send and print.
//!
//! The first paragraph of the lorem ipsum text: something with words in it,
//! long enough that a line arriving one word at a time is visibly arriving one
//! word at a time, and familiar enough that a reader can tell at a glance
//! whether what came out is what went in.
//!
//! # The shape of it
//!
//! One element per line, each element the words on that line. Nothing holds a
//! line terminator: the end of a word array *is* the end of the line, and what
//! stands for it on the wire is [`TERMINATOR`], written once here so that the
//! program sending it and the program reading it cannot disagree about what
//! ends a line.
//!
//! Words and not a line of text, because that is what the example sends: one
//! packet per word, which is what makes the arrangement worth looking at --
//! the words arrive as separate data and the ground puts the line back
//! together. A line kept as a string would have to be split by whoever sent
//! it, and then the splitting would be the thing under test rather than the
//! sending.
//!
//! ```
//! let first = simple_app::lines().next().expect("a paragraph has lines");
//! assert_eq!(first.words().next(), Some("Lorem"));
//! ```

/// What ends a line, as a packet of its own.
///
/// The example's one convention: the words of a line arrive as packets, and
/// then this does. A reader that has it knows the line is whole without being
/// told how many words to expect.
pub const TERMINATOR: &str = "\n";

/// The paragraph: one element per line, each the words on that line.
///
/// A line's words are its space-separated items, so no element of an inner
/// array holds a space -- which is what lets the ground put a line back
/// together by writing one space between them.
pub const PARAGRAPH: &[&[&str]] = &[
    &[
        "Lorem",
        "ipsum",
        "dolor",
        "sit",
        "amet,",
        "consectetur",
        "adipiscing",
        "elit,",
        "sed",
        "do",
    ],
    &[
        "eiusmod",
        "tempor",
        "incididunt",
        "ut",
        "labore",
        "et",
        "dolore",
        "magna",
        "aliqua.",
    ],
    &[
        "Ut",
        "enim",
        "ad",
        "minim",
        "veniam,",
        "quis",
        "nostrud",
        "exercitation",
        "ullamco",
    ],
    &[
        "laboris",
        "nisi",
        "ut",
        "aliquip",
        "ex",
        "ea",
        "commodo",
        "consequat.",
        "Duis",
    ],
    &[
        "aute",
        "irure",
        "dolor",
        "in",
        "reprehenderit",
        "in",
        "voluptate",
        "velit",
    ],
    &[
        "esse",
        "cillum",
        "dolore",
        "eu",
        "fugiat",
        "nulla",
        "pariatur.",
        "Excepteur",
    ],
    &[
        "sint",
        "occaecat",
        "cupidatat",
        "non",
        "proident,",
        "sunt",
        "in",
        "culpa",
        "qui",
    ],
    &[
        "officia", "deserunt", "mollit", "anim", "id", "est", "laborum.",
    ],
];

/// One line of the paragraph.
///
/// A line is its words and nothing else: see [`TERMINATOR`] for what ends one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line(&'static [&'static str]);

impl Line {
    /// The words of this line, in order.
    pub fn words(self) -> Words {
        Words(self.0.iter())
    }

    /// The words as the array they are, for a caller that wants to index them
    /// or count them without walking.
    pub fn as_slice(self) -> &'static [&'static str] {
        self.0
    }

    /// How many words are on it.
    pub fn len(self) -> usize {
        self.0.len()
    }

    /// Whether it has none, which no line of this paragraph has: a line of no
    /// words would be a blank line, and a paragraph is what is between those.
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
}

impl IntoIterator for Line {
    type Item = &'static str;
    type IntoIter = Words;

    fn into_iter(self) -> Words {
        self.words()
    }
}

/// The words of one line, in order.
#[derive(Debug, Clone)]
pub struct Words(std::slice::Iter<'static, &'static str>);

impl Iterator for Words {
    type Item = &'static str;

    fn next(&mut self) -> Option<&'static str> {
        self.0.next().copied()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl ExactSizeIterator for Words {}

impl DoubleEndedIterator for Words {
    fn next_back(&mut self) -> Option<&'static str> {
        self.0.next_back().copied()
    }
}

/// The lines of the paragraph, in order.
#[derive(Debug, Clone)]
pub struct Lines(std::slice::Iter<'static, &'static [&'static str]>);

impl Iterator for Lines {
    type Item = Line;

    fn next(&mut self) -> Option<Line> {
        self.0.next().map(|words| Line(words))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

impl ExactSizeIterator for Lines {}

impl DoubleEndedIterator for Lines {
    fn next_back(&mut self) -> Option<Line> {
        self.0.next_back().map(|words| Line(words))
    }
}

/// Every line of the paragraph, in order.
pub fn lines() -> Lines {
    Lines(PARAGRAPH.iter())
}

/// The line at this number, counting from one as a person reading a file does.
///
/// `None` for a number the paragraph has no line for, nought included: a
/// caller asking for line nought has counted from the wrong end, and answering
/// with the first line would hide that.
pub fn line(number: usize) -> Option<Line> {
    number
        .checked_sub(1)
        .and_then(|at| PARAGRAPH.get(at))
        .map(|words| Line(words))
}

/// How many lines there are.
pub fn len() -> usize {
    PARAGRAPH.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every line has words, and a word is one word.
    ///
    /// The invariant the whole example rests on: the ground writes one space
    /// between what arrives, so a word holding a space would come back as two
    /// words and nobody could tell which packet it had been.
    #[test]
    fn every_line_is_words_with_no_spaces_in_them() {
        assert!(
            len() >= 2,
            "a paragraph of one line would not show wrapping"
        );
        for (at, line) in lines().enumerate() {
            assert!(!line.is_empty(), "line {} has no words", at + 1);
            for word in line.words() {
                assert!(!word.is_empty(), "line {} has an empty word", at + 1);
                assert!(
                    !word.contains(char::is_whitespace),
                    "line {}: \"{word}\" is not one word",
                    at + 1
                );
            }
        }
    }

    /// The iterators say what the arrays say.
    #[test]
    fn the_iterators_walk_the_paragraph() {
        assert_eq!(lines().count(), PARAGRAPH.len());
        assert_eq!(lines().len(), PARAGRAPH.len());

        for (line, words) in lines().zip(PARAGRAPH.iter()) {
            assert_eq!(line.as_slice(), *words);
            assert_eq!(line.words().collect::<Vec<&str>>(), words.to_vec());
            assert_eq!(line.len(), words.len());
            // A line is its words whichever way it is walked, and walkable
            // twice: an iterator that consumed the paragraph would make the
            // second reader's answer depend on the first.
            assert_eq!(line.words().next_back(), words.last().copied());
            assert_eq!(
                line.into_iter().collect::<Vec<&str>>(),
                line.words().collect::<Vec<&str>>()
            );
        }
        assert_eq!(
            lines().next_back().map(Line::as_slice),
            PARAGRAPH.last().copied()
        );
    }

    /// A line is asked for by the number a person would call it.
    #[test]
    fn a_line_is_counted_from_one() {
        assert_eq!(line(1), lines().next());
        assert_eq!(line(len()), lines().next_back());
        assert_eq!(line(0), None, "there is no line nought");
        assert_eq!(line(len() + 1), None);
    }

    /// The first line is the one the example sends, and this is it.
    ///
    /// Written out rather than derived, so that a change to the paragraph that
    /// changes what the example sends is a change somebody has to make here as
    /// well -- and the example's own test, which expects these words, says the
    /// same thing a third time from the far end of a socket.
    #[test]
    fn the_first_line_is_the_beginning_of_the_text() {
        let first = line(1).expect("a paragraph has a first line");
        assert_eq!(
            first.words().collect::<Vec<&str>>().join(" "),
            "Lorem ipsum dolor sit amet, consectetur adipiscing elit, sed do"
        );
    }

    /// And the whole paragraph is the text it is meant to be.
    #[test]
    fn the_paragraph_is_the_lorem_ipsum_text() {
        let said = lines()
            .map(|line| line.words().collect::<Vec<&str>>().join(" "))
            .collect::<Vec<String>>()
            .join(TERMINATOR);
        let whole = said.replace(TERMINATOR, " ");
        assert!(whole.starts_with("Lorem ipsum dolor sit amet,"), "{whole}");
        assert!(whole.ends_with("mollit anim id est laborum."), "{whole}");
        // Four sentences, which is what the paragraph is.
        assert_eq!(whole.matches(". ").count() + 1, 4, "{whole}");
    }
}
