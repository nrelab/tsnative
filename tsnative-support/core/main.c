#include <stdio.h>

#include "runtime.h"

void tsnative_print_number(double value) {
    printf("%.17g\n", value);
}

int main(void) {
    tsnative_print_number(tsnative_main());
    return 0;
}
