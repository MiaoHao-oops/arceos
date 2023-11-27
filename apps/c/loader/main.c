#include <stdio.h>

extern int putchar(int c);

int main()
{
    printf("%p\n", putchar);
    return 0;
}