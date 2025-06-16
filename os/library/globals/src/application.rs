#[derive(Debug, Clone)]
pub struct ApplicationRegistry {
    pub applications: &'static [Application],
}

#[derive(Debug, Clone)]
pub struct Application {
    /// Name of the application (Example: '<git>')
    pub command: &'static str,
    /// Subroutine or mode of the application (Example: 'git <commit>', or 'git <branch>')
    pub sub_commands: &'static [&'static str],
    /// Option of the application, that requires a value, commonly '-KEY VALUE' (Example: 'git commit <-m> <"My message">')
    pub short_flags: &'static [(&'static str, &'static [&'static str])],
    /// Option of the application, that doesn't require a value, commonly '--KEY' (Example: 'git merge master <--no-ff>')
    pub long_flags: &'static [&'static str],
}

/**
 * Register applications here, for the shell interpret them.
 */

pub const APPLICATION_REGISTRY: ApplicationRegistry = ApplicationRegistry {
    applications: &[
        Application {
            command: "test",
            sub_commands: &["arg1", "arg2", "arg3"],
            short_flags: &[
                ("-f", &["flag-1", "flag-2", "flag-3"]),
                ("-m", &["'message 1'", "'message 2'", "'message 3'"]),
            ],
            long_flags: &[],
        },
        //////////////////////
        // Extern Applications
        Application {
            command: "shell",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "date",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "hello",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "helloc",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "keytest",
            sub_commands: &["cooked", "mixed", "raw"],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "legacy_shell",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "ls",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "ntest",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "uptime",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        /////////////////
        // Shell BuildIns
        Application {
            command: "alias",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "cd",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "clear",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "echo",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "exit",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "mkdir",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "pwd",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        Application {
            command: "unalias",
            sub_commands: &[],
            short_flags: &[],
            long_flags: &[],
        },
        //////////////////////////
        // Window Manager BuildIns
        // TODO SUPPORT
    ],
};
