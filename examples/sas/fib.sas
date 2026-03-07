move r0, 0
move r1, 1
move r2, 0

loop:
    gt r3, r2, 10
    jumpif r3, end_loop

    move r4, r0
    add r0, r0, r1
    move r1, r4

    add r2, r2, 1
    jump loop
end_loop:
    halt
