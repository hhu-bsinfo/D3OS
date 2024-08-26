/* ╔═════════════════════════════════════════════════════════════════════════╗
   ║ Module: lib                                                             ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Descr.: Syscalls for the naming service.                                ║
   ╟─────────────────────────────────────────────────────────────────────────╢
   ║ Author: Michael Schoettner, 21.8.2024, HHU                              ║
   ╚═════════════════════════════════════════════════════════════════════════╝
*/

pub enum Errno {
    ENOENT = 2, 	    /* No such file or directory */
    EACCES = 13,	    /* Permission denied */
    EEXIST = 17,	    /* File/directory exists */
    ENOTDIR = 20,	    /* Not a directory */
    EINVAL = 22,	    /* Invalid argument */
    ENOTEMPTY = 90,	    /* Directory not empty */
}
