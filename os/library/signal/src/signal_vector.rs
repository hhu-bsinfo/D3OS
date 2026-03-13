#[repr(u8)]
#[derive(PartialEq, PartialOrd, Copy, Clone, Debug)]
#[allow(dead_code)]
pub enum SignalVector {
    /* Hardware exceptions. Comments show POSIX default action and description. Source: man 7 signal
     * Default actions:
     *   TERM: terminate the process.
     *   IGN:  ignore the signal.
     *   CORE: terminate the process and dump core (see core(5))
     *   STOP: stop the process
     *   CONT: continue the process if currently stopped
     */
    SIGHUP = 1, // TERM, Hangup detected on controlling terminal or death of controlling process
    SIGINT = 2, // TERM, Interrupt from keyboard
    SIGQUIT = 3, // CORE, Quit from keyboard
    SIGILL = 4, // CORE, Illegal Instruction
    SIGTRAP = 5, // CORE, Trace/breakpoint trap
    SIGABRT = 6, // CORE, Abort signal from abort(3)
    //SIGIOT = 6, // CORE, IOT trap. A synonym for SIGABRT
    SIGBUS = 7, // CORE, Bus error (bad memory access)
    SIGFPE = 8, // CORE, Floating point exception
    SIGKILL = 9, // TERM, Kill signal
    SIGUSR1 = 10, // TERM, User-defined signal 1
    SIGSEGV = 11, // CORE, Invalid memory reference
    SIGUSR2 = 12, // TERM, User-defined signal 2
    SIGPIPE = 13, // TERM, Broken pipe: write to pipe with no readers
    SIGALRM = 14, // TERM, Timer signal from alarm(2)
    SIGTERM = 15, // TERM, Termination signal
    SIGSTKFTL = 16, // TERM, Stack fault on coprocessor (unused)
    SIGCHLD = 17, // IGN, Child stopped or terminated
    SIGCONT = 18, // CONT, Continue if stopped
    SIGSTOP = 19, // STOP, Stop process
    SIGTSTP = 20, // STOP, Stop typed at terminal
    SIGTTIN = 21, // STOP, Terminal input for background process
    SIGTTOU = 22, // STOP, Terminal output for background process
    SIGURG = 23, // IGN, Urgent condition on socket (4.2BSD)
    SIGXCPU = 24, // CORE, CPU time limit exceeded (4.2BSD); see setrlimit(2)
    SIGXFSZ = 25, // CORE, File size limit exceeded (4.2BSD); see setrlimit(2)
    SIGVTALRM = 26, // TERM, Virtual alarm clock (4.2BSD)
    SIGPROF = 27, // TERM, Profiling timer expired
    SIGWINCH = 28, // IGN, Window resize signal (4.3BSD, Sun)
    SIGIO = 29, // TERM, I/O now possible (4.2BSD)
    SIGPWR = 30, // TERM, Power failure (System V)
    SIGSYS = 31, // CORE, Bad system call (SVr4); see also seccomp(2)
    //SIGUNUSED = 31, // CORE, Synonymous with SIGSYS
}

use self::SignalVector::*; // Make signals directly available in this namespace so we can omit the prefix `SignalVector::` each time

const SIGNAL_VECTORS: [SignalVector; 31] = [
    SIGHUP, SIGINT, SIGQUIT, SIGILL, SIGTRAP, SIGABRT, SIGBUS, SIGFPE,
    SIGKILL, SIGUSR1, SIGSEGV, SIGUSR2, SIGPIPE, SIGALRM, SIGTERM,
    SIGSTKFTL, SIGCHLD, SIGCONT, SIGSTOP, SIGTSTP, SIGTTIN, SIGTTOU,
    SIGURG, SIGXCPU, SIGXFSZ, SIGVTALRM, SIGPROF, SIGWINCH, SIGIO,
    SIGPWR, SIGSYS
];

pub const MAX_VECTORS: usize = 32;

impl TryFrom<u8> for SignalVector {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value < 32 && value > 0 {
            Ok(SIGNAL_VECTORS[value as usize - 1])
        } else {
            Err(())
        }
    }
}
