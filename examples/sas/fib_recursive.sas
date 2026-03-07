    push 0
    push 10
    call fib, 8, 8
    pop r0
    halt

fib:
    move r0, [bp+8]
    lte r1, r0, 1
    jumpif r1, fib_base

    push r0

    sub r2, r0, 1
    push 0
    push r2
    call fib, 8, 8
    pop r3
    push r3

    move r0, [bp+32]
    sub r2, r0, 2
    push 0
    push r2
    call fib, 8, 8
    pop r4

    move r3, [bp+40]
    add r5, r3, r4
    move [bp+0], r5

    pop r3
    pop r0
    ret 8, 8

fib_base:
    move [bp+0], r0
    ret 8, 8
