/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - stdlib.h: abs(), abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 *   - time.h: struct tm
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */


#ifndef _STDLIB_H_
#define _STDLIB_H_

#include <stddef.h>

#ifndef NULL
#define NULL ((void*)0)
#endif

void* malloc(size_t size);
void* calloc(size_t num, size_t size);
void* realloc(void *ptr, size_t new_size);
void free(void *ptr);

int atoi(const char *str);
long atol(const char *str);
double atof (const char *str);
long strtol(const char *str, char **endptr, int base);
float strtof(const char *str, char **endptr);
double strtod(const char *str, char **endptr);

int abs(int n);

void bsearch(const void *key, const void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *));
void qsort(void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *));

int system(const char *command);
void abort(void);
void exit(int exit_code);

#endif