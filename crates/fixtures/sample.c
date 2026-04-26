/* Sample C module for testing */
#include <stdio.h>
#include <stdlib.h>

int helper(int x) {
    return x * 2;
}

int process(int *items, int len) {
    /* FIXME: check for null pointer */
    int total = 0;
    for (int i = 0; i < len; i++) {
        total += helper(items[i]);
    }
    return total;
}
