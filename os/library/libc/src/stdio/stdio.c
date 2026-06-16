#include <stdio.h>

int vprintf(const char *format, va_list vlist) {
	return vfprintf(stdout, format, vlist);
}

int printf(const char *format, ...) {
	va_list list;

	va_start(list, format);
	const int ret = vprintf(format, list);
	va_end(list);

	return ret;
}

int fprintf(FILE *stream, const char *format, ...) {
    va_list list;

    va_start(list, format);
    const int ret = vfprintf(stream, format, list);
    va_end(list);

    return ret;
}

int snprintf(char *buffer, size_t bufsz, const char *format, ...) {
	va_list list;

	va_start(list, format);
	const int ret = vsnprintf(buffer, bufsz, format, list);
	va_end(list);

	return ret;
}

int sscanf(const char *s, const char *format, ...) {
	va_list args;

	va_start(args, format);
	const int ret = vsscanf(s, format, args);
	va_end(args);

	return ret;
}