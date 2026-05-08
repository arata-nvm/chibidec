# chibidec

```c
// gcc -O0 fizzbuzz.c -o fizzbuzz
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
```

```shell
% cargo run testdata/fizzbuzz
// Id { idx: 11 } [0x100000460-0x100000478]
0x100000460: sub sp, sp, #0x20
0x100000464: stp x29, x30, [sp, #0x10]
0x100000468: add x29, sp, #0x10
0x10000046c: stur wzr, [x29, #-4]
0x100000470: mov w8, #1
0x100000474: str w8, [sp, #8]
0x100000478: b #0x10000047c
while (w8 <= 0xf) {
    // Id { idx: 5 } [0x10000047c-0x100000484]
    0x10000047c: ldr w8, [sp, #8]
    0x100000480: subs w8, w8, #0xf
    0x100000484: b.gt #0x100000524
    // Id { idx: 1 } [0x100000488-0x100000488]
    0x100000488: b #0x10000048c
    // Id { idx: 4 } [0x10000048c-0x1000004a0]
    0x10000048c: ldr w8, [sp, #8]
    0x100000490: mov w10, #3
    0x100000494: sdiv w9, w8, w10
    0x100000498: mul w9, w9, w10
    0x10000049c: subs w8, w8, w9
    0x1000004a0: cbnz w8, #0x1000004b8
    if (w8 != 0) {
        // Id { idx: 7 } [0x1000004b8-0x1000004cc]
        0x1000004b8: ldr w8, [sp, #8]
        0x1000004bc: mov w10, #5
        0x1000004c0: sdiv w9, w8, w10
        0x1000004c4: mul w9, w9, w10
        0x1000004c8: subs w8, w8, w9
        0x1000004cc: cbnz w8, #0x1000004e4
        if (w8 == 0) {
            // Id { idx: 0 } [0x1000004d0-0x1000004d0]
            0x1000004d0: b #0x1000004d4
            // Id { idx: 12 } [0x1000004d4-0x1000004e0]
            0x1000004d4: adrp x0, #0x100000000
            0x1000004d8: add x0, x0, #0x545
            0x1000004dc: bl #0x100000534
            0x1000004e0: b #0x100000500
        } else {
            // Id { idx: 13 } [0x1000004e4-0x1000004fc]
            0x1000004e4: ldr w8, [sp, #8]
            0x1000004e8: mov x9, sp
            0x1000004ec: str x8, [x9]
            0x1000004f0: adrp x0, #0x100000000
            0x1000004f4: add x0, x0, #0x54a
            0x1000004f8: bl #0x100000534
            0x1000004fc: b #0x100000500
        }
        // Id { idx: 6 } [0x100000500-0x100000500]
        0x100000500: b #0x100000504
    } else {
        // Id { idx: 3 } [0x1000004a4-0x1000004a4]
        0x1000004a4: b #0x1000004a8
        // Id { idx: 10 } [0x1000004a8-0x1000004b4]
        0x1000004a8: adrp x0, #0x100000000
        0x1000004ac: add x0, x0, #0x540
        0x1000004b0: bl #0x100000534
        0x1000004b4: b #0x100000504
    }
    // Id { idx: 8 } [0x100000504-0x100000510]
    0x100000504: adrp x0, #0x100000000
    0x100000508: add x0, x0, #0x54d
    0x10000050c: bl #0x100000534
    0x100000510: b #0x100000514
    // Id { idx: 2 } [0x100000514-0x100000520]
    0x100000514: ldr w8, [sp, #8]
    0x100000518: add w8, w8, #1
    0x10000051c: str w8, [sp, #8]
    0x100000520: b #0x10000047c
    continue;
}
// Id { idx: 9 } [0x100000524-0x100000530]
0x100000524: mov w0, #0
0x100000528: ldp x29, x30, [sp, #0x10]
0x10000052c: add sp, sp, #0x20
0x100000530: ret
```
