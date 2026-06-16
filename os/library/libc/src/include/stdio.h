#ifndef _STDIO_H_
#define _STDIO_H_

#include <stdarg.h>
#include <stddef.h>

#ifndef NULL
#define NULL ((void*)0)
#endif

#define SEEK_SET 0
#define SEEK_CUR 1
#define SEEK_END 2

typedef size_t FILE;

extern FILE *stdout;
extern FILE *stderr;

extern FILE* fopen(const char *filename, const char *mode);
extern int fclose(FILE *stream);
extern size_t fread(void *buffer, size_t size, size_t count, FILE *stream);
extern size_t fwrite(const void *buffer, size_t size, size_t count, FILE *stream);
extern int fflush(FILE *stream);
extern int fseek(FILE *stream, long offset, int origin);
extern long ftell(FILE *stream);

int remove(const char *pathname);
int rename(const char *old_filename, const char *new_filename);

int putchar(int ch);
int puts(const char *str);

int vfprintf(FILE *stream, const char *format, va_list vlist);
int vsnprintf(char *buffer, size_t bufsz, const char *format, va_list vlist);

int printf(const char *format, ...);
int fprintf(FILE *stream, const char *format, ...);
int snprintf(char *buffer, size_t bufsz, const char *format, ...);

int sscanf(const char *s, const char *format, ...);
int vsscanf(const char *s, const char *format, const va_list args);

#endif