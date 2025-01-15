#include "../../../library/libc/src/runtime.h"

int main(int argc, char *argv[]) {
    terminal_write("Hello from C!\n\n");

    terminal_write("Arguments:\n");
    for (int i = 0; i < argc; i++) {
        terminal_write("  ");
        terminal_write(argv[i]);
        terminal_write("\n");
    }

    return 0;
}