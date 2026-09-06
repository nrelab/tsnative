#include <stdio.h>

extern double tsnative_main(void);

int main(void) {
    printf("%.17g\n", tsnative_main());
    return 0;
}
