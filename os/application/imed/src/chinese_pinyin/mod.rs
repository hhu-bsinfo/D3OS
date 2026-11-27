use alloc::{collections::BTreeMap, string::String};
use core::fmt::Write;
use ime::{DecodedKey, KeyCode};

const PAGE_SIZE: usize = 5;
const MAX_SPELLING: usize = 32;
const MAX_WORD_BYTES: usize = 48;

pub struct Dictionary(BTreeMap<&'static str, &'static str>);

impl Dictionary {
    pub fn load() -> Self {
        Self(include_str!("dictionary.tsv").lines().filter_map(|line| line.split_once('\t')).collect())
    }

    fn candidates(&self, spelling: &str) -> impl Iterator<Item = &'static str> + use<> {
        self.0.get(spelling).copied().into_iter().flat_map(|words| words.split('\t'))
    }
}

pub struct Pinyin {
    chinese: bool,
    spelling: String,
    page: usize,
}

#[derive(Default)]
pub struct Result {
    pub pass: bool,
    pub committed: String,
}

impl Default for Pinyin {
    fn default() -> Self {
        Self {
            chinese: true,
            spelling: String::new(),
            page: 0,
        }
    }
}

impl Pinyin {
    pub fn chinese(&self) -> bool {
        self.chinese
    }

    pub fn cancel(&mut self) {
        self.spelling.clear();
        self.page = 0;
    }

    pub fn toggle(&mut self) {
        self.cancel();
        self.chinese = !self.chinese;
    }

    pub fn key(&mut self, key: DecodedKey, dictionary: &Dictionary) -> Result {
        let mut result = Result::default();
        if !self.chinese {
            result.pass = true;
            return result;
        }

        match key {
            DecodedKey::Unicode(c @ ('a'..='z' | 'A'..='Z')) => {
                if self.spelling.len() < MAX_SPELLING {
                    self.spelling.push(c.to_ascii_lowercase());
                    self.page = 0;
                }
            }
            _ if self.spelling.is_empty() => result.pass = true,
            DecodedKey::Unicode('\x08') => {
                self.spelling.pop();
                self.page = 0;
            }
            DecodedKey::Unicode('\x1b') => self.cancel(),
            DecodedKey::Unicode('\n') => {
                result.committed = core::mem::take(&mut self.spelling);
                self.page = 0;
            }
            DecodedKey::RawKey(KeyCode::PageUp) | DecodedKey::Unicode('-') => {
                self.page = self.page.saturating_sub(1);
            }
            DecodedKey::RawKey(KeyCode::PageDown) | DecodedKey::Unicode('=') => {
                if self.candidate(dictionary, PAGE_SIZE).is_some() {
                    self.page += 1;
                }
            }
            DecodedKey::Unicode(c @ (' ' | '1'..='5')) => {
                let index = if c == ' ' { 0 } else { c as usize - '1' as usize };
                if let Some(word) = self.candidate(dictionary, index) {
                    result.committed.push_str(word);
                    self.cancel();
                } else if c == ' ' {
                    result.committed = core::mem::take(&mut self.spelling);
                    result.committed.push(' ');
                    self.page = 0;
                }
            }
            DecodedKey::Unicode(c) if !c.is_control() => {
                if let Some(word) = self.candidate(dictionary, 0) {
                    result.committed.push_str(word);
                } else {
                    result.committed.push_str(&self.spelling);
                }
                result.committed.push(c);
                self.cancel();
            }
            _ => {
                self.cancel();
                result.pass = true;
            }
        }
        result
    }

    fn page_words(&self, dictionary: &Dictionary) -> impl Iterator<Item = &'static str> + use<> {
        dictionary
            .candidates(&self.spelling)
            .filter(|word| word.len() <= MAX_WORD_BYTES)
            .skip(self.page * PAGE_SIZE)
    }

    fn candidate(&self, dictionary: &Dictionary, index: usize) -> Option<&'static str> {
        self.page_words(dictionary).nth(index)
    }

    pub fn preview(&self, dictionary: &Dictionary) -> String {
        if self.spelling.is_empty() {
            return String::new();
        }
        let mut preview = self.spelling.clone();
        for (index, word) in self.page_words(dictionary).take(PAGE_SIZE).enumerate() {
            // Limit the preview independently of what gets committed.
            let _ = write!(preview, " {}.", index + 1);
            preview.extend(word.chars().take(6));
        }
        preview
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn commits_simplified_words_and_passes_english() {
        let dictionary = Dictionary::load();
        let mut pinyin = Pinyin::default();
        for c in "nihao".chars() {
            assert!(!pinyin.key(DecodedKey::Unicode(c), &dictionary).pass);
        }
        assert!(pinyin.preview(&dictionary).contains("1.你好"));
        assert_eq!(pinyin.key(DecodedKey::Unicode(' '), &dictionary).committed, "你好");
        for c in "hanyu".chars() {
            pinyin.key(DecodedKey::Unicode(c), &dictionary);
        }
        assert_eq!(pinyin.key(DecodedKey::Unicode('2'), &dictionary).committed, "韩语");
        pinyin.toggle();
        assert!(!pinyin.chinese());
        assert!(pinyin.key(DecodedKey::Unicode('A'), &dictionary).pass);
    }

    #[test]
    fn edits_pages_and_cancels_preedit() {
        let dictionary = Dictionary::load();
        let mut pinyin = Pinyin::default();
        for c in "shix\x08".chars() {
            pinyin.key(DecodedKey::Unicode(c), &dictionary);
        }
        let first = pinyin.preview(&dictionary);
        pinyin.key(DecodedKey::RawKey(KeyCode::PageDown), &dictionary);
        assert_ne!(pinyin.preview(&dictionary), first);
        pinyin.key(DecodedKey::RawKey(KeyCode::PageUp), &dictionary);
        assert_eq!(pinyin.preview(&dictionary), first);
        pinyin.key(DecodedKey::Unicode('\x1b'), &dictionary);
        assert!(pinyin.preview(&dictionary).is_empty());
    }
}
