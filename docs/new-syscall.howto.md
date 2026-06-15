## Adding a new system call

### Work to be done in User Mode

1. You need to add an entry for your new system call in: `os/library/syscall`
In `enum SysCall`. Insert your new system call name before `LastEntryMarker`.

2. For using your new system call, see examples in `os/library`. A system call is typically used within a runtime library function, not directly within an application. *Important: Check params before firing a system call and abort if they are not valid.*

3. Return values for system calls are defined in `return_vals.rs`. This file is used by user and kernel mode parts.


### Work to be done in Kernel Mode

1. You need to add the counterpart implementation for your user mode system call in a file in `os/kernel/syscall`, e.g. `sys_naming.rs`. All syscall function names should start with the prefix `sys_`. *Important: It is of upmost important to have the correct signature and check params.* 

2. Add your system call to the `SyscallTable` in `kernel/syscall/syscall_dispatcher.rs`.

3. The return value is `isize` and created by calling `convert_syscall_result_to_ret_code` which expects a `SyscallResult`, defined in `return_vals.rs` (see above). So kernel counterparts should use `SyscallResult` which is converted to `isize` when returning to user mode.

