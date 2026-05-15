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
while (<= 0) {
    block2:
      0x10000047c: t4.1 = add sp.1, 0x8
      0x10000047c: w8.2 = load [t4.1]
      0x100000480: w8.3 = sub w8.2, 0xf
      0x100000484: if > 0 goto 0x100000524
    block8:
      0x100000488: goto 0x10000048c
    block0:
      0x10000048c: t5.1 = add sp.1, 0x8
      0x10000048c: w8.4 = load [t5.1]
      0x100000490: w10.1 = copy 0x3
      0x100000494: w9.1 = sdiv w8.4, w10.1
      0x100000498: w9.2 = sdiv w9.1, w10.1
      0x10000049c: w8.5 = sub w8.4, w9.2
      0x1000004a0: if w8.5 != 0 goto 0x1000004b8
    if (w8.5 != 0) {
        block7:
          0x1000004b8: t6.1 = add sp.1, 0x8
          0x1000004b8: w8.8 = load [t6.1]
          0x1000004bc: w10.2 = copy 0x5
          0x1000004c0: w9.3 = sdiv w8.8, w10.2
          0x1000004c4: w9.4 = sdiv w9.3, w10.2
          0x1000004c8: w8.9 = sub w8.8, w9.4
          0x1000004cc: if w8.9 != 0 goto 0x1000004e4
        if (w8.9 == 0) {
            block5:
              0x1000004d0: goto 0x1000004d4
            block1:
              0x1000004d4: x0.5 = copy 0x100000000
              0x1000004d8: x0.6 = add x0.5, 0x545
              0x1000004dc: call 0x100000534
              0x1000004e0: goto 0x100000500
        } else {
            block10:
              0x1000004e4: t7.1 = add sp.1, 0x8
              0x1000004e4: w8.10 = load [t7.1]
              0x1000004e8: x9.1 = copy sp.1
              0x1000004ec: t8.1 = add x9.1, 0x0
              0x1000004ec: store [t8.1], x8.0
              0x1000004f0: x0.7 = copy 0x100000000
              0x1000004f4: x0.8 = add x0.7, 0x54a
              0x1000004f8: call 0x100000534
              0x1000004fc: goto 0x100000500
        }
        block9:
          0x100000500: goto 0x100000504
    } else {
        block12:
          0x1000004a4: goto 0x1000004a8
        block11:
          0x1000004a8: x0.3 = copy 0x100000000
          0x1000004ac: x0.4 = add x0.3, 0x540
          0x1000004b0: call 0x100000534
          0x1000004b4: goto 0x100000504
    }
    block4:
      0x100000504: x0.1 = copy 0x100000000
      0x100000508: x0.2 = add x0.1, 0x54d
      0x10000050c: call 0x100000534
      0x100000510: goto 0x100000514
    block3:
      0x100000514: t9.1 = add sp.1, 0x8
      0x100000514: w8.6 = load [t9.1]
      0x100000518: w8.7 = add w8.6, 0x1
      0x10000051c: t10.1 = add sp.1, 0x8
      0x10000051c: store [t10.1], w8.7
      0x100000520: goto 0x10000047c
    continue;
}
block6:
  0x100000524: w0.1 = copy 0x0
  0x100000528: t11.1 = add sp.1, 0x10
  0x100000528: x29.2 = load [t11.1]
  0x100000528: t12.1 = add sp.1, 0x18
  0x100000528: x30.1 = load [t12.1]
  0x10000052c: sp.2 = add sp.1, 0x20
  0x100000530: ret
```
