#include <stdio.h>

int main() {
    for (int i = 1; i <= 15; i++) {
        if (i % 3 == 0) printf("fizz");
        else if (i % 5 == 0) printf("buzz");
        else printf("%d", i);
        printf("\n");
    }
    return 0;
}
