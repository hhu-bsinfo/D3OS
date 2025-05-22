#[derive(Debug)]
pub struct ApplicationRegistry<const N: usize> {
    pub applications: [Application; N],
}

#[derive(Debug)]
pub struct Application {
    pub name: &'static str,
}

/**
 * Register applications here, for the shell interpret them.
 * Note: Update N to the number of  
 */

pub const APPLICATION_REGISTRY: ApplicationRegistry<17> = ApplicationRegistry {
    applications: [
        //////////////////////
        // Extern Applications
        Application { name: "shell" },
        Application { name: "date" },
        Application { name: "hello" },
        Application { name: "helloc" },
        Application { name: "keytest" },
        Application {
            name: "legacy_shell",
        },
        Application { name: "ls" },
        Application { name: "ntest" },
        Application { name: "uptime" },
        /////////////////
        // Shell BuildIns
        Application { name: "alias" },
        Application { name: "cd" },
        Application { name: "clear" },
        Application { name: "echo" },
        Application { name: "exit" },
        Application { name: "mkdir" },
        Application { name: "pwd" },
        Application { name: "unalias" },
        //////////////////////////
        // Window Manager BuildIns
        // TODO SUPPORT
    ],
};
