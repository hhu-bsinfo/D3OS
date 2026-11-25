#![no_std]

extern crate alloc;

mod chinese_pinyin;

#[cfg(not(test))]
use runtime as _;

#[cfg(not(test))]
use {
    chinese_pinyin::{Dictionary, Pinyin},
    concurrent::thread,
    ime::pinyin::{CHINESE, Mailbox, PASS, RESET, TOGGLE},
};

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn main() {
    let dictionary = Dictionary::load();
    let Some(mailbox) = Mailbox::create() else {
        return;
    };
    let mut pinyin = Pinyin::default();

    loop {
        let Some(request) = mailbox.receive() else {
            thread::sleep(2);
            continue;
        };
        if request.flags & RESET != 0 {
            pinyin.cancel();
        }

        let result = if request.flags & TOGGLE != 0 {
            pinyin.toggle();
            chinese_pinyin::Result::default()
        } else if let Some(key) = request.key {
            pinyin.key(key, &dictionary)
        } else {
            chinese_pinyin::Result {
                pass: true,
                ..Default::default()
            }
        };
        let preview = pinyin.preview(&dictionary);
        let flags = (if pinyin.chinese() { CHINESE } else { 0 }) | (if result.pass { PASS } else { 0 });
        mailbox.reply(request, flags, &result.committed, &preview);
    }
}
