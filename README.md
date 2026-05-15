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

### cfg

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

### ssa

```
fn _main {
block13 _main:
  0x100000460: sp.1 = sub sp.0, 0x20
  0x100000464: t0.1 = add sp.1, 0x10
  0x100000464: store [t0.1], x29.0
  0x100000464: t1.1 = add sp.1, 0x18
  0x100000464: store [t1.1], x30.0
  0x100000468: x29.1 = add sp.1, 0x10
  0x10000046c: t2.1 = add x29.1, 0xfffffffffffffffc
  0x10000046c: store [t2.1], wzr.0
  0x100000470: w8.1 = copy 0x1
  0x100000474: t3.1 = add sp.1, 0x8
  0x100000474: store [t3.1], w8.1
  0x100000478: goto 0x10000047c

block10:
  t10.1 = phi block2:t10.2, block13:t10.0
  x0.1 = phi block2:x0.11, block13:x0.0
  x9.1 = phi block2:x9.4, block13:x9.0
  t5.1 = phi block2:t5.2, block13:t5.0
  t9.1 = phi block2:t9.2, block13:t9.0
  t6.1 = phi block2:t6.3, block13:t6.0
  w10.1 = phi block2:w10.4, block13:w10.0
  t8.1 = phi block2:t8.4, block13:t8.0
  w9.1 = phi block2:w9.6, block13:w9.0
  t7.1 = phi block2:t7.4, block13:t7.0
  w8.2 = phi block2:w8.13, block13:w8.1
  t4.1 = phi block2:t4.2, block13:t4.0
  0x10000047c: t4.2 = add sp.1, 0x8
  0x10000047c: w8.3 = load [t4.2]
  0x100000480: w8.4 = sub w8.3, 0xf
  0x100000484: if > 0 goto 0x100000524

block0:
  0x100000488: goto 0x10000048c

block7:
  0x10000048c: t5.2 = add sp.1, 0x8
  0x10000048c: w8.5 = load [t5.2]
  0x100000490: w10.2 = copy 0x3
  0x100000494: w9.2 = sdiv w8.5, w10.2
  0x100000498: w9.3 = sdiv w9.2, w10.2
  0x10000049c: w8.6 = sub w8.5, w9.3
  0x1000004a0: if w8.6 != 0 goto 0x1000004b8

block9:
  0x1000004a4: goto 0x1000004a8

block3:
  0x1000004a8: x0.7 = copy 0x100000000
  0x1000004ac: x0.8 = add x0.7, 0x540
  0x1000004b0: call 0x100000534
  0x1000004b4: goto 0x100000504

block5:
  0x1000004b8: t6.2 = add sp.1, 0x8
  0x1000004b8: w8.7 = load [t6.2]
  0x1000004bc: w10.3 = copy 0x5
  0x1000004c0: w9.4 = sdiv w8.7, w10.3
  0x1000004c4: w9.5 = sdiv w9.4, w10.3
  0x1000004c8: w8.8 = sub w8.7, w9.5
  0x1000004cc: if w8.8 != 0 goto 0x1000004e4

block1:
  0x1000004d0: goto 0x1000004d4

block12:
  0x1000004d4: x0.4 = copy 0x100000000
  0x1000004d8: x0.5 = add x0.4, 0x545
  0x1000004dc: call 0x100000534
  0x1000004e0: goto 0x100000500

block11:
  0x1000004e4: t7.2 = add sp.1, 0x8
  0x1000004e4: w8.9 = load [t7.2]
  0x1000004e8: x9.2 = copy sp.1
  0x1000004ec: t8.2 = add x9.2, 0x0
  0x1000004ec: store [t8.2], x8.0
  0x1000004f0: x0.2 = copy 0x100000000
  0x1000004f4: x0.3 = add x0.2, 0x54a
  0x1000004f8: call 0x100000534
  0x1000004fc: goto 0x100000500

block6:
  x0.6 = phi block11:x0.3, block12:x0.5
  x9.3 = phi block11:x9.2, block12:x9.1
  t8.3 = phi block11:t8.2, block12:t8.1
  t7.3 = phi block11:t7.2, block12:t7.1
  w8.10 = phi block11:w8.9, block12:w8.8
  0x100000500: goto 0x100000504

block8:
  x0.9 = phi block3:x0.8, block6:x0.6
  x9.4 = phi block3:x9.1, block6:x9.3
  t6.3 = phi block3:t6.1, block6:t6.2
  w10.4 = phi block3:w10.2, block6:w10.3
  t8.4 = phi block3:t8.1, block6:t8.3
  w9.6 = phi block3:w9.3, block6:w9.5
  t7.4 = phi block3:t7.1, block6:t7.3
  w8.11 = phi block3:w8.6, block6:w8.10
  0x100000504: x0.10 = copy 0x100000000
  0x100000508: x0.11 = add x0.10, 0x54d
  0x10000050c: call 0x100000534
  0x100000510: goto 0x100000514

block2:
  0x100000514: t9.2 = add sp.1, 0x8
  0x100000514: w8.12 = load [t9.2]
  0x100000518: w8.13 = add w8.12, 0x1
  0x10000051c: t10.2 = add sp.1, 0x8
  0x10000051c: store [t10.2], w8.13
  0x100000520: goto 0x10000047c

block4:
  0x100000524: w0.1 = copy 0x0
  0x100000528: t11.1 = add sp.1, 0x10
  0x100000528: x29.2 = load [t11.1]
  0x100000528: t12.1 = add sp.1, 0x18
  0x100000528: x30.1 = load [t12.1]
  0x10000052c: sp.2 = add sp.1, 0x20
  0x100000530: ret

}
```
