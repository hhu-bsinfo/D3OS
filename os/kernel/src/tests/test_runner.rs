use spin::{Once, Mutex};
use alloc::vec::Vec;
use alloc::boxed::Box;
use alloc::vec;

static GLOBAL_TEST_RUNNER: Once<Mutex<TestRunner>> = Once::new();

pub fn get_test_runner() -> Option<&'static Mutex<TestRunner>> {
    GLOBAL_TEST_RUNNER.get()
}

pub trait TestPlugin: Send + Sync {
    type Output = ();
    fn run(&self) -> Self::Output;
}

pub struct TestRunner {
    entrypoints: Vec<Box<dyn TestPlugin<Output = ()> + Send + Sync>>,
}

impl TestRunner {
    pub fn new() {
        GLOBAL_TEST_RUNNER.call_once(|| {
            let entrypoints: Vec<Box<dyn TestPlugin<Output = ()> + Send + Sync>> = vec![];
            Mutex::new(TestRunner { entrypoints })
        });
    }

    pub fn exec(&self) {
        for plugin in &self.entrypoints {
            plugin.run();
        }
    }
}

