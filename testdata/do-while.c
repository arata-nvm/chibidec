#include <stdio.h>

int main() {
    int sum = 0;
    int i = 0; 
    do {
      sum += i;
      i++;
    } while (i <= 10);
    return sum;
}
