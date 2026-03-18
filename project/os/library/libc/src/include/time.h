/*
 * The following C standard library functions were originally implemented
 * as part of a bachelor's thesis, written by Gökhan Cöpcü:
 *   - math.h: abs()
 *   - stdlib.h: abort(), atoi(), strtol(), bsearch(), qsort()
 *   - string.h: strcat(), strcmp(), strcpy()
 * The original source code can be found here: https://git.hhu.de/bsinfo/thesis/ba-gocoe100
 */

#ifndef _TIME_H_
#define _TIME_H_

struct tm {
    int tm_sec; // seconds after the minute [0-60]
    int tm_min; // minutes after the hour [0-59]
    int tm_hour; // hours since midnight [0-23]
    int tm_mday; // day of the month [1-31]
    int tm_mon; // months since January [0-11]
    int tm_year; // years since 1900
    int tm_wday; // days since Sunday [0-6]
    int tm_yday; // days since January 1 [0-365]
    int tm_isdst; // Daylight Saving Time flag
};

#endif
