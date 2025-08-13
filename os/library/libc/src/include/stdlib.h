/*
 * The C standard library is based on a bachelor's thesis, written by Gökhan Cöpcü.
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

#ifndef _STDLIB_H_
#define _STDLIB_H_

#include <stddef.h>

void abort(void);

int atoi(const char *str);
long atol(const char *str);
long strtol(const char *str, char **endptr, int base);

void qsort(void *base, size_t nmemb, size_t size, int (*compar)(const void *, const void *));

#endif