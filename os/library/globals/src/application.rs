#[derive(Debug, Clone)]
pub struct Application {
    pub namespace: &'static str,
    pub single_value: &'static [&'static str],
    pub key_value_pair: &'static [(&'static str, &'static [&'static str])],
}

/**
 * Register applications here, for the shell interpret them.
 */
pub const APPLICATION_REGISTRY: &'static [Application] = &[
    //////////////////////
    // Debug
    Application {
        namespace: "test",
        single_value: &[
            "arg1",
            "arg2",
            "arg3",
            "--long-flag-1",
            "--long-flag-2",
            "--long-flag-3",
            "x=1",
            "x=2",
            "x=3",
        ],
        key_value_pair: &[
            ("-f", &["flag-1", "flag-2", "flag-3"]),
            ("-m", &["'message 1'", "'message 2'", "'message 3'"]),
        ],
    },
    //////////////////////
    // Extern Applications
    Application {
        namespace: "shell",
        single_value: &["--no-history", "--no-auto-completion"],
        key_value_pair: &[],
    },
    Application {
        namespace: "date",
        single_value: &[],
        key_value_pair: &[],
    },
    Application {
        namespace: "hello",
        single_value: &["ARG"],
        key_value_pair: &[],
    },
    Application {
        namespace: "helloc",
        single_value: &["ARG"],
        key_value_pair: &[],
    },
    Application {
        namespace: "keytest",
        single_value: &["cooked", "mixed", "raw"],
        key_value_pair: &[],
    },
    Application {
        namespace: "legacy_shell",
        single_value: &[],
        key_value_pair: &[],
    },
    Application {
        namespace: "ls",
        single_value: &[],
        key_value_pair: &[],
    },
    Application {
        namespace: "ntest",
        single_value: &[],
        key_value_pair: &[],
    },
    Application {
        namespace: "uptime",
        single_value: &[],
        key_value_pair: &[],
    },
    /////////////////
    // Shell BuildIns
    Application {
        namespace: "alias",
        single_value: &["KEY=VALUE"],
        key_value_pair: &[],
    },
    Application {
        namespace: "cd",
        single_value: &["DIR"],
        key_value_pair: &[],
    },
    Application {
        namespace: "clear",
        single_value: &[],
        key_value_pair: &[],
    },
    Application {
        namespace: "echo",
        single_value: &["ARG"],
        key_value_pair: &[],
    },
    Application {
        namespace: "exit",
        single_value: &[],
        key_value_pair: &[],
    },
    Application {
        namespace: "mkdir",
        single_value: &["DIR"],
        key_value_pair: &[],
    },
    Application {
        namespace: "pwd",
        single_value: &[],
        key_value_pair: &[],
    },
    Application {
        namespace: "unalias",
        single_value: &["'KEY'"],
        key_value_pair: &[],
    },
    Application {
        namespace: "theme",
        single_value: &["d3os", "boring", "debug"],
        key_value_pair: &[],
    },
    Application {
        namespace: "window_manager",
        single_value: &[],
        key_value_pair: &[],
    },
];
