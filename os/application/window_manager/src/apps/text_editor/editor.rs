use crate::apps::text_editor::config::TextEditorConfig;
use crate::apps::text_editor::messages::Message;
use crate::apps::text_editor::model::Document;
use crate::apps::text_editor::view::View;
use alloc::{rc::Rc, string::String, vec::Vec};
use graphic::bitmap::Bitmap;
use spin::rwlock::RwLock;
use text_buffer::TextBuffer;

static CODE_EXAMPLE: &str = r#"
// Calculate fac!
int main() {
    int n;
    unsigned long long factorial = 1;

    printf("Enter a positive integer: ");
    scanf("%d", &n);

    if (n < 0) {
        printf("Error: Factorial is not defined for negative integers.\n");
        return 1;
    }

    for (int i = 1; i <= n; ++i) {
        factorial *= i;
    }

    printf("Factorial of %d is %llu\n", n, factorial);
    return 0;
}
[package]
name = "syntax"
version = "0.1.0"
edition = "2024"
authors = ["Julius Carl Drodofsky <julius@drodofsky.xyz>"]

[features]
default = ["alloc"]
alloc = []


[dependencies]

[dependencies.nom]
version = "8"
default-features = false
features = ["alloc"]



echo -n "Enter a number: "
read num

if test "$num" -lt 0
    exit 1
end

result = 1
counter = $num

while test "$counter" -gt 1
    result = (expr $result \* $counter)
    counter = (expr $counter - 1)
end

echo "Factorial of $num is $result"
"#;
static MARKDOWN_EXAMPLE: &str = r#"
# Heading 1

## Heading 2

This is a paragraph with **bold text** and *italic text*.

---

Another paragraph after a horizontal rule.

Some **Strong** Text.

Some *Emphasis* Text.

### Heading3

- Unordered item 1  
- Unordered item 2  
  - Nested unordered item  
  - Another nested item  

1. Ordered item 1  
2. Ordered item 2  
   1. Nested ordered item  
   2. Another nested item
"#;

const KEYWORDS: &[&str] = &[
    "int",
    "return",
    "for",
    "if",
    "end",
    "while",
    "unsigned",
    "long",
    "package",
    "dependencies",
    "features",
    "echo",
    "read",
];

pub fn apply_message(
    documents: &Rc<RwLock<OpenDocuments>>,
    canvas: &Rc<RwLock<Bitmap>>,
    msg: Message,
) {
    if documents.write().current().is_none() {
        return;
    }
    documents.write().current().unwrap().update(msg);
    let mut msg = View::render(&documents.write().current().unwrap(), &mut canvas.write());
    while msg.is_some() {
        documents
            .write()
            .current()
            .unwrap()
            .update(Message::ViewMessage(msg.unwrap()));
        msg = View::render(&documents.write().current().unwrap(), &mut canvas.write());
    }
}

pub struct OpenDocuments<'a, 'b> {
    documents: Vec<Document<'a, 'b>>,
    current: usize,
}

impl<'a, 'b> OpenDocuments<'a, 'b> {
    pub fn new() -> OpenDocuments<'a, 'b> {
        OpenDocuments {
            documents: Vec::new(),
            current: 0,
        }
    }

    pub fn insert(&mut self, document: Document<'a, 'b>) {
        self.documents.push(document);
    }

    pub fn current(&mut self) -> Option<&mut Document<'a, 'b>> {
        self.documents.get_mut(self.current)
    }

    pub fn next(&mut self) -> Option<&mut Document<'a, 'b>> {
        self.current += 1;
        if self.documents.iter().len() == 0 {
            return None;
        }
        self.current %= self.documents.len();
        self.documents.get_mut(self.current)
    }

    pub fn prev(&mut self) -> Option<&mut Document<'a, 'b>> {
        self.current = self
            .current
            .checked_sub(1)
            .unwrap_or(self.documents.len().checked_sub(1).unwrap_or(0));
        self.documents.get_mut(self.current)
    }
    pub fn dummy() -> OpenDocuments<'a, 'b> {
        let mut ret = OpenDocuments::new();
        let text_buffer = TextBuffer::from_str(CODE_EXAMPLE);
        ret.insert(Document::new(
            Some(String::from("Code.clike")),
            text_buffer,
            TextEditorConfig::new(900, 600, &KEYWORDS),
        ));
        let text_buffer = TextBuffer::from_str(MARKDOWN_EXAMPLE);
        ret.insert(Document::new(
            Some(String::from("ReadMe.md")),
            text_buffer,
            TextEditorConfig::new(900, 600, &KEYWORDS),
        ));
        ret
    }
}
