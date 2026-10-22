section .text
global disable_interrupts

; Clears the interrupt flag.
disable_interrupts:
    cli
    ret
