/*
 * The C standard library is based on a bachelor's thesis, written by Gökhan Cöpcü.
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

#ifndef _STRING_H_
#define _STRING_H_

#include <stddef.h>

int memcmp(const void *s1, const void *s2, size_t n);
void *memcpy(void *dest, const void *src, size_t n);
void *memmove(void *dest, const void *src, size_t n);
void *memset(void *dest, int c, size_t n);

char *strcat(char *dest, const char *src);
int strcmp(const char *s1, const char *s2);
char *strcpy(char *dest, const char *src);
size_t strlen(const char *s);

#endif