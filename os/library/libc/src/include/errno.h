#ifndef _ERRNO_H_
#define _ERRNO_H_

#define EDOM 1
#define ERANGE 2
#define EILSEQ 3
#define EISDIR 4
#define ENOENT 5
#define EEXIST 6
#define EBADF 7
#define EACCES 8

extern int error;

#define errno error

#endif